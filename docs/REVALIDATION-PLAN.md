# Fleet revalidation plan over the open product register

Research 2026-11-20 gaps, B2505, Pro part. Module `src/revalidation.rs`, command
`kshana-pro revalidation plan` (`src/cli/revalidation_cmd.rs`), tests in `tests/signed_dossier.rs`
(module `revalidation_plan`). Feature `pro-revalidation-plan` (Pro).

**What is open and what is here.** The open engine has the product register
(`kshana.register/1`: products scored under a stated method, each with `revalidate_by`) and
reads the issuer's signed revocation list (`kshana.register-crl/1`). `kshana register view`
joins one register and one list at a date: each entry is `current`, `due` or `revoked`, and
revocation is `unknown`, `checked` or `stale`. Publishers sign registers with the open
`kshana register sign`; the issuer signs the list. Nothing in this crate signs either document.
What this command adds is the plan across many registers and entries.

The documents are read by their documented format. The signature covers the schema id, a
newline, then the compact JSON in struct order with an empty `signature`, and is Ed25519 by the
key in `signer`. A list's issuer key is required; a register's publisher key is optional and
pins the signature when given. The published revocation-list test vector is checked in the
tests. TODO: switch to the open types once they are on the open engine's main branch (#159).

## Request

```toml
title = "Fleet revalidation, Q1"
warn_days = 60        # DUE SOON within this many days of the revalidation date
lead_days = 90        # re-scoring starts this many days before it, unless the maker has its own

[[lead]]
maker = "Maker Two"
days = 120

[[register]]
file = "lab-a/register.json"
key = "<publisher key hex>"      # optional pin
crl = "lab-a/crl.json"           # optional
crl_key = "<issuer key hex>"     # required with crl
```

## Plan

`kshana-pro revalidation plan request.toml --out <dir> --signer <name> --key-file <file>
[--date YYYY-MM-DD]` checks every register and list (stopping on any that does not verify) and
plans at the date (default: the signing date).

* Per entry: the open status and revocation state, with the open semantics: revoked from a
  revocation dated on or before the plan date (a stale list's revocations still apply), due
  after `revalidate_by`, current otherwise. Each entry also gets the start date (`revalidate_by`
  less the lead time), the days left, and a flag: `OVERDUE`, `DUE SOON`, `SCHEDULED` or
  `REVOKED`.
* Per register: publisher, signer, the list used and its next update, the revocation state,
  and list ids naming no entry of the register.
* By month of the revalidation date: entries per flag. By maker: counts per flag and the next
  start and revalidation dates.

It writes and signs `report.json` (`kshana-pro-revalidation-plan/1`), `report.html`, `plan.csv`,
`plan.ics` (for each entry not revoked, all-day events on the start date and on the revalidation
date), the request, and every register and list read.

MODELLED: a plan from the registers supplied. An entry says only that a product was scored
under a method. The plan says when it is to be scored again.
