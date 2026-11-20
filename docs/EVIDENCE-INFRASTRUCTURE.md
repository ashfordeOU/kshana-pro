# Evidence infrastructure: observation provenance and long-term signatures

Research 2026-11-20 gaps, epic E25 (items B2501 and B2503). Two commands, both verifying
offline, both MODELLED in the sense that every key, authority and batch in this repository is
a test fixture: no real feeder network or time-stamp authority has been used with them, and
nothing here is certified or accredited by anyone.

## Signed observation provenance (`kshana-pro obs-provenance`, B2501)

A feeder signs each `kshana.obs/1` batch it sends. The signature block is a sidecar,
`<batch>.prov.json` (format `kshana-obs-provenance/1`), so the batch bytes are unchanged and
every `kshana.obs/1` reader, the open `kshana obs check` included, reads signed and unsigned
batches alike.

The Ed25519 signature covers, in canonical JSON:

| field | meaning |
|---|---|
| `feeder_id`, `public_key`, `key_id` | who fed the batch |
| `signed_utc` | the feeder's own clock when it signed (a claim, not a time-stamp) |
| `station_id`, `location` | the station; the position copied from the batch header (`source: header`), or declared by the feeder when the header has none (`source: declared`) |
| `batch_sha256`, `batch_bytes` | the exact batch bytes |
| `epochs`, `alarms`, `corrections`, `first_t_s`, `last_t_s`, `start_label` | what the batch holds |
| `seq`, `prev_batch_sha256` | the feeder's batch counter and the hash of its previous batch |

The network operator keeps a feeder keyring (`kshana-feeder-keyring/1`): for each feeder, its
keys with `valid_from`, optional `valid_until` and optional `revoked_since`.

```
kshana-pro obs-provenance keygen                                   # on the feeder
kshana-pro obs-provenance keyring-add keyring.json --feeder tll-01 --public-key <hex> --valid-from 2026-11-01T00:00:00Z
kshana-pro obs-provenance sign b0007.jsonl --feeder tll-01 --key-file feeder.key --seq 7 --prev b0006.jsonl   # Pro
kshana-pro obs-provenance verify batches/ --keyring keyring.json [--require-signed] [--json]
```

`verify` reports each batch as `signed` (unchanged, signature holds, a keyring key of that
feeder, valid at the stated signing time, position agrees with the header), `unsigned` (no
sidecar: still checked as `kshana.obs/1` and readable; refused only with `--require-signed`)
or `rejected` (with the reasons), and checks each feeder's sequence: no repeated number, no
gap, each batch naming its predecessor's hash.

What it proves: the holder of the feeder key produced these exact bytes. It does not prove the
observations are true, and `signed_utc` is the feeder's clock. Signing is feature
`pro-obs-provenance` (Pro); keygen, keyring-add and verify are free.

## Long-term signatures (`kshana-pro longterm`, B2503)

A signed dossier (`signature.json`, see `kshana-pro dossier`) gets a long-term record,
`longterm.json` (format `kshana-longterm/1`), beside the signature. The dossier signature does
not cover it; it protects itself. It follows the idea of the archive time-stamp chains of
RFC 4998 (Evidence Record Syntax) and the archive time-stamps of the ETSI long-term signature
levels, but it is its own JSON format, not an implementation of either.

The record holds:

- **The signer's key history** (`kshana-key-history/1`): a self-signed genesis event (first
  key, valid from) and one event per rotation, signed by both the outgoing and the incoming
  key and chained by hash to the event before. A verifier who pins any key of the history
  reaches all the others. A rotation's `reason` is `scheduled` or `compromise`; its
  `effective_utc` ends the outgoing key's validity (for a compromise, the time it may have
  begun).
- **An archive time-stamp chain**: stamp 0 is an RFC 3161 token over the exact bytes of
  `signature.json`. Stamp *n* covers this text, built from stamp *n-1*:

  ```
  kshana-longterm/1 renewal
  seq=<n>
  hash_alg=sha256
  prev_imprint=<stamp n-1 imprint, hex>
  prev_token_sha256=<SHA-256 of stamp n-1's token>
  prev_chain_sha256=<SHA-256 of each stored chain certificate, comma-separated>
  ```

  Each stamp stores the token, any further chain certificates, and the authority's root
  certificate, so the record verifies with no network. The root is trusted only when the
  verifier pins it (`--tsa-anchor <file>` or `--tsa-fingerprint <sha256>`).

```
kshana-pro longterm key-genesis --key-file signer.key --holder "Example Ltd" --valid-from 2026-01-01T00:00:00Z --out history.json   # Pro
kshana-pro longterm request dossier/ --out req.tsq                 # free: the next RFC 3161 request
kshana-pro longterm add-token dossier/ --token reply.tsr --tsa-anchor tsa-root.pem --tsa-name "Authority A" --history history.json   # Pro
kshana-pro longterm stamp dossier/ --tsa-url https://tsa.example/ --tsa-anchor tsa-root.pem   # Pro, optional network path via system curl
kshana-pro longterm key-rotate history.json --from-key-file old.key --to-key-file new.key --effective 2027-01-01T00:00:00Z   # Pro
kshana-pro longterm history dossier/ --history history.json        # Pro: record the extended history
kshana-pro longterm verify dossier/ --trust <any key of the history> --tsa-anchor a.pem,b.pem [--at 2045-01-01T00:00:00Z] [--json]
```

`verify` checks, offline: every file and the dossier signature; the key history; that the
dossier key is in it and a pinned key reaches it; each token's CMS signature and chain to a
pinned root (the open engine's RFC 3161 verifier) and its imprint over the data this format
says it covers; that each renewal was stamped before the previous stamp's certificates expired
(the earliest `notAfter` among the token's and chain's certificates, a conservative reading);
that the first stamp's time falls inside the dossier key's validity; and that the last stamp's
certificates are still valid at `--at` (default: now). It prints `renew by` and advises
renewal within a year of that date.

So verification still succeeds after the signer rotates its key (the dossier was stamped while
the old key was valid) and after a change of time-stamp authority (the new authority stamps the
old stamp); it fails when the dossier was first stamped after its key was retired or reported
compromised, and when the chain was not renewed in time.

Not checked: certificate revocation (no CRL or OCSP is fetched or stored yet), a weakening of
SHA-256 (the hash is named per stamp; only SHA-256 is accepted today), name and policy
constraints, the token nonce. A time-stamp shows when data existed; it says nothing about
whether the evidence is correct.

The standalone `scripts/verify-dossier-signature.py` ignores `longterm.json` as it ignores
`signature.json`.

## Not built yet (E25 follow-ups)

- B2502 public append-only transparency log with Merkle roots anchored by two authorities
  (hosted service side).
- B2504 customer evidence vault (legal hold, multi-year dashboards, compliance calendar,
  regulator templates, open export).
- B2505 validated-product register, yearly revalidation, certificate revocation list, per-test
  uncertainty budgets.
- B2506 versioned verdict API and insurer portal (hosted service side).
- B2508 explicit operating-envelope block in every Pro output.
- Revocation data (CRL/OCSP) stored in the long-term record; hash-algorithm renewal.
- Verification of provenance sidecars in the open `kshana obs check`.

Fixtures: `tests/fixtures/longterm/gen.sh` makes two test authorities with OpenSSL (one whose
certificate lasts 40 days, one 20 years) and stamps the committed test dossier; it writes the
renewal text in shell, independently of the Rust code, so the tests cross-check the format.
