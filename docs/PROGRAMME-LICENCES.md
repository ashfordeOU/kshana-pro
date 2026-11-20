# Programme licences: feeder, host, teaching, bench

Research backlog E39, item B3902 (sources 09 A4, A38; 09b D59, D69, H64). Source:
`src/license/programme.rs`, the `programme` checks in `src/license/mod.rs` (`verify_with`,
`issue_v2`) and `src/cli/license_cmd.rs`. Tests: `tests/oem_licensing.rs` and the unit tests of
the module.

Four licence kinds that grant Pro in return for something other than a subscription, plus a
telemetry opt-in. They are licence data and validation: no new feature, no new way around a
check. Whether and to whom each is offered is the owner's decision.

## Kinds and their explicit tiers

| Kind | For | Highest tier | Products | Life | Seats | Gate |
|---|---|---|---|---|---|---|
| `feeder` | a person whose Kshana Station feeds observations | Pro | cli, desktop, station | ≤ 366 days | ≤ 1 | fresh feed receipt |
| `host` | a site hosting one or more stations | Pro | cli, desktop, station | ≤ 366 days | ≤ one per listed station | fresh feed receipt |
| `teaching` | a course at a named institution | Pro | cli, desktop | ≤ 366 days | any | none |
| `bench` | a lab bench running Pro bench drivers | Enterprise | cli, station | any | ≤ 1 | bound key (TPM or software) |

The table is `programme::POLICIES`: one entry per kind, checked by a test. A programme token may
not carry a grant above its kind's highest tier, nor ever `onprem-contract`, `pro-attack-iq` or
`pro-ks-inside`, nor the vetted-buyer, trial or special entitlement flags.

## Token format

A programme licence is a **v3** token: the v2 claims plus a `programme` block:

```json
{"licensee":"…","tier":"pro","expires_unix":…,"v":3,"products":["cli","station"],
 "programme":{"kind":"feeder","stations":["ee-tll-01"]}}
```

* A build that reads only v2 refuses v3 ("newer than this build reads"), so an older build can
  never run a programme token as an ordinary licence without its rules.
* Verification checks the block against the kind's policy (`programme::validate`) every time, not
  only at issue: a token signed outside `issue_v2` that breaks its policy is refused.
* A v3 token without a programme, or a programme in a v2 token, is refused.
* Ordinary tokens are unchanged on the wire (still v2, no new fields).

Issuing (vendor):

```sh
kshana-pro license issue --tier pro --licensee "A Feeder" --days 365 --product cli,station \
    --programme feeder --programme-station ee-tll-01 --register register.jsonl
kshana-pro license issue --tier pro --licensee "Example University" --days 180 --product cli,desktop \
    --programme teaching --institution "Example University" --course "GNSS resilience" --seats 40
kshana-pro license issue --tier enterprise --licensee "Example Lab" --product cli,station \
    --feature pro-eloran-hw --programme bench --bench-key ed25519:<64 hex> --bench-class software
```

## Fail closed

`verify_with` returns a **gated** programme token (feeder, host, bench) with an **Evaluation**
entitlement. Only `programme::feed_entitlement` (a fresh receipt) or
`programme::bench_entitlement` (the bound key answers a challenge) returns what the token names.
Every caller that knows nothing about programmes (pack delivery, the node-locked method gate,
online renewal) therefore sees Evaluation. The CLI lifts a gated licence in one place
(`apply_programme` in `src/cli/license_cmd.rs`), and only while the licence is active (not
expired, in its renewal window or revoked).

## Feeder and host: feed receipts

The ingest service signs, per station, the time it last received valid observations:

```json
{"format":"kshana-pro-feed-receipt/1","licence":"kt_…","station":"ee-tll-01",
 "last_obs_unix":…,"issued_unix":…,"kid":"…"}
```

signed with the licence key (`payload_hex.signature_hex`, like a token). Pro holds while the
newest receipt for a listed station is at most **48 hours** past its last observation
(`FEED_GRACE_SECS`); after that the licence is at Evaluation until a newer receipt is installed.
Receipts for another licence, an unlisted station, from an untrusted key, edited after signing, or
dated in the future are ignored. A host licence reports which listed stations are active.

```sh
kshana-pro license feed-receipt issue --licence kt_… --station ee-tll-01 --last-obs <unix>   # vendor
kshana-pro license feed-receipt install receipt.txt                                          # holder
kshana-pro license programme [--json]
```

Not built here: the ingest side that decides a feed is valid and issues receipts automatically,
and automatic receipt download (the licence store's opt-in online refresh would carry it).

## Teaching

Granted directly (no gate), but only within its policy: Pro at most, cli and desktop only, a named
institution and course, and an expiry within 366 days. Watermarking teaching outputs as
"not for operational use" is a follow-up.

## Bench: key binding

A bench token names a key (`<algorithm>:<SHA-256 of the public key>`) and the weakest class it
accepts. At run time a `BenchBinder` signs a fresh challenge (token ID, time, 16 random bytes)
and the signature is checked against the named key; a binder of a weaker class than the token
requires is refused.

* **Software binder (`software` class) — weaker.** An Ed25519 key in the licence store
  (`bench-key.seed`, mode 0600). Anyone who can read the file can copy the binding to another
  machine. `kshana-pro license bench-key init` creates it and prints its ID for the vendor;
  `bench-key show` prints it again.
* **TPM binder (`tpm` class) — interface only.** The interface accepts P-256 ECDSA keys, the
  curve every TPM 2.0 supports, and is tested with a software P-256 key standing in for one. **No
  TPM binder is part of this build**, so a `tpm`-class token cannot be satisfied here. Building
  one (a pure-Rust TPM 2.0 command path over `/dev/tpmrm0` or the Windows TBS, with no C
  dependency, behind a cargo feature) and testing it on hardware is a follow-up; until then
  nothing should describe bench licences as TPM-protected.

A bench licence does not replace machine binding: a token may carry both, and the machine check
applies as before. The node-locked patent-candidate methods still need a machine-bound licence
with the on-premises contract grant, which no programme token can carry.

## Telemetry opt-in (simulator vendors)

`telemetry_opt_in` is a claim of any v2 or v3 token: the holder agreed to share anonymised
scenario outcomes. `programme::telemetry_record` returns a record only when the claim is set:
scenario kind, outcome word, a duration bucket, the week number and the build version: no licensee,
machine, input values or exact times. `append_outbox` writes it to a local JSON Lines file for the
holder to review and send under their agreement. **Nothing is sent automatically**; Kshana Pro
still phones home only for the opt-in online refresh. Wiring the record into specific commands
is a follow-up.
