# Product certificates: revocation status and yearly revalidation

Research 2026-11-20 gaps, B2505, Pro part. Module `src/cert_status.rs`, command `kshana-pro cert`
(`src/cli/cert_cmd.rs`), tests in `tests/signed_dossier.rs` (module `cert_status`). Feature
`pro-cert-revalidation` (Pro; `cert status` is free).

**The split.** The validated-product register is public and open. Issuing the certificate
revocation list is the hosted service's job. This crate holds yearly revalidation and the
status check. It reads and verifies revocation lists and never issues one. The tests sign their
lists with a fixture key.

**What already existed.** The method parts of B2505: the precision study
([METHOD-VALIDATION.md](METHOD-VALIDATION.md)), and per-test budgets and the re-scoring schedule
([METHOD-UNCERTAINTY.md](METHOD-UNCERTAINTY.md)). Certificates, their status and the
revalidation calendar are new.

## Certificate

`kshana-pro cert make --id <id> --product <name> --method <id> --method-version <v> --issued
<date> [--due <date>] [--scope <text>] --out <dir> --signer <name> --key-file <file>` writes
`certificate.json` (`kshana-pro-cert-product/1`) and signs the pack. The certificate holds the ID,
subject product, method and version, issue date, revalidation-due date and issuer (the signer).
Revalidation is due twelve months after issue unless `--due` is given. Month ends are clamped:
29 February falls due on 28 February.

## Status

`kshana-pro cert status <certificate-dir> [--list <file> --trust <issuer key hex>] [--cert-trust
<lab key hex>] [--at <utc>] [--json]` exits 0 only when the state is `valid`.

The revocation list file is `payload_hex.signature_hex`, like the licence revocation list. The
payload is JSON `kshana-pro-cert-revocations/1` with `issuer`, `sequence`, `issued_utc`,
`next_update_utc`, `revoked` (`id`, `reason`, `revoked_utc`) and `kid`. The signature is Ed25519
over the payload by the key whose ID is `kid`, which must be one of the `--trust` keys.

| state | when |
|---|---|
| `invalid` | the certificate pack does not verify (changed file, or not signed by a `--cert-trust` key) |
| `revoked` | the verified list names the certificate with a revocation time at or before `--at`; a stale list's revocation still counts |
| `revalidation-overdue` | not revoked, and `--at` is after the revalidation-due date (known even without a list) |
| `unknown` | no list, a list that does not verify, or a list past its `next_update_utc`: a stale list never reports `valid` |
| `valid` | none of the above |

## Revalidation calendar

`kshana-pro cert schedule <dir>[,...] --out <dir> --signer <name> --key-file <file> [--list ...
--trust ...] [--warn-days 60]` writes and signs `report.json` (`kshana-pro-cert-calendar/1`),
`report.html`, `calendar.csv` and `calendar.ics` (one all-day event per revalidation). It covers
every certificate with its due date, days left and a flag: `OVERDUE`, `DUE SOON` (within the
warning days), `SCHEDULED`, or `REVOKED` (no revalidation event).

MODELLED: a status of the records supplied. Whether a product is fit for use is for the
certificate's issuer to say.
