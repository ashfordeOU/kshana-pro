# A9: integration security for the Pro service

Status: decisions 1 to 5 built on 6 Nov 2026 (`src/serve/{auth,oidc,audit}.rs`; tests `service_security`,
`service_oidc`); the rest as noted. Roadmap item A9 (`docs/plans/ROADMAP-0.41-0.43.md`): OIDC, tenant isolation,
audit log, retention, PII-free crowd ingest. Done when the K2 pen test passes.

## What exists

`kshana-pro serve --api-token-file` sets one shared bearer token. Only its SHA-256 is held, and it is
compared in constant time. There are no users, roles, tenants or audit (`docs/SERVICE.md`, "A bearer token is
not user management").

## Decisions

1. **Callers are principals.** Every request is made by a principal: `{id, tenant, role}`. Principals come from
   one of two places, and both may be configured at once:
   - **API keys** (`--keys-file keys.toml`): named keys for machines. The file holds only the SHA-256 of each key,
     never the key itself (`kshana-pro keys add` prints a new key once and writes its hash). The old
     `--api-token-file` still works: it is an `admin` key with id `token` in tenant `default`.
   - **OIDC bearer tokens** (`--oidc oidc.toml`): JWTs from the operator's identity provider. They are verified
     offline against a JWKS file the operator places next to the config. The service fetches nothing at run time,
     so it works air-gapped; rotating the keys means replacing the file, which is re-read when it changes. Checked:
     signature (ES256 and RS256; `none` and HMAC algorithms are refused), `iss`, `aud`, `exp` and `nbf` (60 s
     leeway), and a key id that is in the set. The role and tenant come from configurable claims; a token without
     them is refused, never defaulted.
2. **Three roles, ordered.** `reader` (GET routes), `submitter` (also POST jobs and studies), `admin` (also ledger
   verification and the audit log). The route table carries the role each route needs, so the OpenAPI document
   states it.
3. **Tenants isolate jobs.** A job records its tenant when submitted. A principal sees, lists and downloads only
   its own tenant's jobs; another tenant's job answers 404, never 403, so job ids do not leak. Job ids include the
   tenant in their hash, so two tenants submitting the same request get different jobs. `admin` in tenant `*`
   (a key only, never an OIDC token) sees every tenant, for operators.
4. **Audit log.** One JSON line per request except `GET /health`, in `<data-dir>/audit/YYYY-MM-DD.jsonl`: time,
   principal id, tenant, role, source (`key` or `oidc`), method, route template (not the raw path or query),
   job id when there is one, status and duration. Never a token, a request body or a client address. Each line
   carries the SHA-256 of the previous line (the first line of a day chains to the last line of the day before),
   so a deleted or edited line breaks the chain. `GET /audit/verify` (admin) checks it. Refused requests (401,
   403) are logged with principal `-`.
5. **Retention.** `--audit-retention-days N` (default 400, so a year plus a margin) deletes whole day files
   older than N days when the server starts, and logs each deletion. Built. Still planned: pruning while the
   server runs (today a restart prunes), and `--job-retention-days` for finished jobs.
6. **PII-free crowd ingest.** Not part of this service step. It belongs with the crowd-sourced interference map,
   which already hashes aircraft and vessel ids and never stores them. A9 adds only the rule that the service
   never logs a client address.

## Not in this step

TLS termination stays with the reverse proxy (`docs/SERVICE.md`). Fetching JWKS over HTTPS, SAML and SCIM are not
planned. The K2 pen test runs on the finished service.

## Checks

- **Keys:** a wrong key, a revoked key, a missing header and a lower role each get the right status.
- **OIDC:** tokens signed by a test key in the JWKS pass. Tampered, expired, not-yet-valid, wrong issuer, wrong
  audience, unknown key, `alg: none` and HS256 tokens are refused. RS256 and ES256 are both checked against
  vectors from an independent implementation (Python `cryptography`) committed as fixtures.
- **Tenants:** a job of tenant A is 404 to tenant B on every job route, and neither the job list nor the
  board shows it.
- **Audit:** every request writes one line. Editing or deleting any line makes verification fail at that line.
  The log never contains a token, a body or an address.
