# The on-premises job service

A free user runs the open Kshana engine on a laptop. A Pro team runs it as a service on
its own network: `kshana-pro serve --data-dir <dir>` keeps a queue of jobs on disk, runs
them on a bounded pool of workers, stores each result and its artefacts under a job
identifier, and records every delivery in a tamper-evident, hash-chained ledger. A
stop or a crash loses no work: on restart, every job that had not finished is queued
again, and a finished job is never run twice.

The service is the same dependency-light server as before: blocking, standard library
only, one request per connection, no asynchronous runtime. It makes no outbound
connection and sends no telemetry.

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| API | application programming interface |
| CCSDS | Consultative Committee for Space Data Systems |
| CI | continuous integration (here: Pack C, the scenario-subscription CI) |
| CSV | comma-separated values |
| DOP | dilution of precision (PDOP: position DOP) |
| DSN | Deep Space Network |
| HTML | HyperText Markup Language |
| HTTP | HyperText Transfer Protocol |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| MiB | mebibyte (1,048,576 bytes) |
| PNT | positioning, navigation and timing |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SVG | Scalable Vector Graphics |
| TLS | Transport Layer Security |
| TOML | Tom's Obvious Minimal Language (the scenario file format) |
| UTC | Coordinated Universal Time |

## Starting the service

```bash
kshana-pro serve --addr 127.0.0.1:8088 --data-dir /var/lib/kshana-pro --workers 2 \
                 --license <token> [--api-token-file /run/secrets/kshana_api_token]
```

| Flag | Meaning |
|---|---|
| `--data-dir <dir>` | Turns the job service on. The job store, the artefacts and the ledger live here. Without it the server answers the routes it answered before, with the same behaviour, plus `GET /openapi.json`. |
| `--workers <n>` | Worker threads for jobs, 1 to 64 (default 1). |
| `--api-token-file <path>` | Every route except `GET /health` then needs `Authorization: Bearer <token>`. The file holds the token on one line. |
| `--license <token>` | The entitlement the whole process serves at (a development build also accepts `--dev`). |

At start the server prints the address it bound, the routes it serves, and how many
unfinished jobs it queued again. It never prints the access token.

## Routes

The router and the OpenAPI 3.1 document (`GET /openapi.json`) are both generated from
one route table (`src/serve/api.rs`, `ROUTES`), so the document lists exactly what is
served. The job routes appear only when the server has a data directory.

| Method | Path | What it does |
|---|---|---|
| GET | `/health` | Liveness probe, `ok`. Never needs the token. |
| GET | `/version` | Overlay version, number of open engine kinds, serving tier. |
| GET | `/kinds` | Open and Pro scenario kinds, with each Pro kind's minimum tier. |
| POST | `/run` | Run one scenario (body: TOML, at most 1 MiB) and wait for the result. |
| POST | `/study` | Validate and acknowledge a study request; with a data directory, also queue its `dossier` job. |
| GET | `/openapi.json` | The OpenAPI 3.1 document. |
| GET | `/` | The job board: a self-contained page listing the jobs. |
| POST | `/jobs` | Submit a job, `{"op": ..., "request": ...}` (body at most 16 MiB). |
| GET | `/jobs` | List jobs, newest first. Query: `limit` (1 to 1000, default 50), `state`. |
| GET | `/jobs/<id>` | One job's status. |
| GET | `/jobs/<id>/result` | The result document of a succeeded job. |
| GET | `/jobs/<id>/artefacts/<name>` | One artefact of a succeeded job. |
| GET | `/ledger` | The delivery ledger as stored. |
| GET | `/ledger/verify` | Verify the ledger and every delivered job's stored files. |

### Status codes

| Code | When |
|---|---|
| 200 | Success; also `POST /jobs` for a job that already exists (nothing new is run). |
| 202 | `POST /jobs` stored and queued a new job. |
| 400 | Malformed JSON; a job identifier that is not 64 lower-case hexadecimal characters; an artefact name that is not a plain file name; a bad query parameter. |
| 401 | The access token is required and missing or wrong. |
| 403 | The entitlement does not allow the operation or the scenario kind. Nothing is queued. The body names the tiers, never the licensee. |
| 404 | No such route, job or artefact. |
| 409 | `result` or `artefacts` of a job that has not finished, or that failed. |
| 413 | Body above the cap (1 MiB; 16 MiB for `POST /jobs`), refused before it is read. |
| 422 | Unknown operation, missing or wrongly typed field, empty body. |

## A recorded session

Everything below is quoted from a real session against a running server on 127.0.0.1,
recorded with raw sockets on 2026-09-30 (the site's showcase keeps the full recording,
with every header and every body). The server ran as

```bash
kshana-pro serve --dev --addr 127.0.0.1:0 --data-dir showcase/data --workers 1
```

with `--dev` so that no licence and no customer or licensee name appears anywhere. The
scenarios are the open engine's own public files.

This session was recorded with Kshana Pro 0.1.0 on open engine 0.28.0. The showcase was
recorded again with Kshana Pro 0.2.0 on open engine 0.29.0 (with `--sample`): the job
identifiers, the constellation job's result (SHA-256 `a60ed771…`, the same figures), the
counts and the ledger check are the same. The times, the ledger's record hashes, the
campaign job's run digest (its results state engine 0.29.0) and the constellation job's
`chart.svg` (the same 141,789 bytes, now SHA-256 `0f3ac477dc10…`) differ, so the times,
hashes and engine version quoted below are the first recording's.

**Submit the 5000-satellite LEO PNT constellation design as a job.** The request body
carries the scenario file (`leo-pnt-mega-shell.toml`, SHA-256
`935a3860dbb7cb70cb64b60089040d642dfdf2cf11b519c47a76d4a96377595f`) as a JSON string:

```http
POST /jobs HTTP/1.1
Host: 127.0.0.1:56175
Content-Type: application/json
Content-Length: 2354

{"op": "run", "request": {"scenario": "# A low-Earth-orbit positioning, navigation and timing (PNT) mega-constellation: 5 000\n# satellites in four Walker shells, ..."}}
```

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Content-Length: 212
Connection: close

{"created":true,"job_id":"e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94","links":{"status":"/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94"},"op":"run","state":"queued"}
```

**Poll it.** The first poll found it running, the second finished. Every time is the
server's clock:

```json
{"artefacts":[],"attempts":1,"error":null,"finished_unix":null,"finished_utc":null,"job_id":"e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94","kind":"constellation-design","label":null,"links":{"status":"/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94"},"op":"run","result_sha256":null,"seq":1,"started_unix":1790791040,"started_utc":"2026-09-30T17:57:20Z","state":"running","submitted_unix":1790791040,"submitted_utc":"2026-09-30T17:57:20Z"}
```

```json
{"artefacts":[{"bytes":141789,"name":"chart.svg","sha256":"e0808ac436e61650d84a92007e16808fad29a5fcce6cadc565c1382b7048c967"}],"attempts":1,"error":null,"evidence_tier":"MODELLED","finished_unix":1790791040,"finished_utc":"2026-09-30T17:57:20Z","job_id":"e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94","kind":"constellation-design","label":"MODELLED - two-body Keplerian orbits (optional secular J2) around a spherical body, geometric visibility above an elevation mask, all-in-view dilution of precision ... (see docs/VERIFICATION-MATRIX.md)","links":{"artefacts":["/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94/artefacts/chart.svg"],"result":"/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94/result","status":"/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94"},"op":"run","result_sha256":"a60ed7715c33975ea3795034fca5d30485aa09602be00778fe742d598568cd64","seq":1,"started_unix":1790791040,"started_utc":"2026-09-30T17:57:20Z","state":"succeeded","submitted_unix":1790791040,"submitted_utc":"2026-09-30T17:57:20Z"}
```

The engine's own evidence label travels with the job (`label`, shortened here with
"..."), and `evidence_tier` is its leading word: VALIDATED, MODELLED or PARTNER. Some
open kinds write a label that does not start with a tier word (the `link-budget` label,
for example, begins "One-way link budget over the CCSDS 401 / DSN 810-005 link
equation"); such a job carries the label as it is and no `evidence_tier`, and the
service assigns it none. Every Pro kind's result starts its label with MODELLED, so its
job carries `evidence_tier` MODELLED. A dossier result carries a label per figure inside
it and a top-level MODELLED label. A `ci` result (`ci.json`) carries a top-level label
that starts with MODELLED, and its `report.html` states MODELLED for the whole report.

**Fetch the result.** `GET /jobs/<id>/result` returned the 194,023-byte result document,
byte for byte the document a direct run of the same file gives (the acceptance test
compares the two). Its global block:

```json
"global": {"availability_pct": 99.2187, "fix_pct": 100.0, "mean_visible": 39.3901, "min_visible": 6,
           "pdop": {"max": 18.0597, "mean": 1.1308, "median": 1.11, "p90": 1.285, "p95": 1.355, "p99": 2.655},
           "worst_site_availability_pct": 0.0, ...}
```

with `"total_satellites": 5000`. `GET /jobs/<id>/artefacts/chart.svg` returned the chart
(`Content-Type: image/svg+xml`, 141,789 bytes, with
`Content-Security-Policy: sandbox; default-src 'none'; ...` so a stored page is shown,
never run).

**Submit the same request again.** The same job, nothing run:

```json
{"created":false,"job_id":"e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94","links":{"status":"/jobs/e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94"},"op":"run","state":"succeeded"}
```

**A scenario the engine rejects** (`kind = "ephemeris"` with no orbit) is accepted as a
job and becomes a failed job carrying the engine's own error; its result is refused:

```json
{"artefacts":[],"attempts":1,"error":"ephemeris: ephemeris scenario needs either a `tle` or an `orbit`","finished_unix":1790791041,"finished_utc":"2026-09-30T17:57:21Z","job_id":"59d27ceda4b0b1a340b8a2cd8f06250d784eec172ba4432296d3b23165a2b62f","kind":"ephemeris","label":null,"links":{"status":"/jobs/59d27ceda4b0b1a340b8a2cd8f06250d784eec172ba4432296d3b23165a2b62f"},"op":"run","result_sha256":null,"seq":2,"started_unix":1790791041,"started_utc":"2026-09-30T17:57:21Z","state":"failed","submitted_unix":1790791041,"submitted_utc":"2026-09-30T17:57:21Z"}
```

```http
HTTP/1.1 409 Conflict
Content-Type: application/json
Content-Length: 228
Connection: close

{"error":"the job failed; it has no result (see error)","job_error":"ephemeris: ephemeris scenario needs either a `tle` or an `orbit`","job_id":"59d27ceda4b0b1a340b8a2cd8f06250d784eec172ba4432296d3b23165a2b62f","state":"failed"}
```

**Queue work behind a long job.** A 200-seed Monte Carlo campaign
(`campaign-monte-carlo-clock-holdover.toml`: scenario hash
`e093a8c2f16933f42eb97c22ff3029df9f50928f31da237318de06ff84444dfd`, seed 20260928,
engine 0.28.0) and three quick runs were submitted to the single worker.
`GET /jobs?limit=100` at that moment returned
`"counts":{"failed":1,"queued":3,"running":1,"succeeded":1}` and `"total":6`, newest
first; the first entry:

```json
{"artefacts":[],"attempts":0,"error":null,"finished_unix":null,"finished_utc":null,"job_id":"4667e03cf3298c5fd333f7a61f5a4117cea515479157241973587c5a3dc7bc89","kind":"passes","label":null,"links":{"status":"/jobs/4667e03cf3298c5fd333f7a61f5a4117cea515479157241973587c5a3dc7bc89"},"op":"run","result_sha256":null,"seq":6,"started_unix":null,"started_utc":null,"state":"queued","submitted_unix":1790791041,"submitted_utc":"2026-09-30T17:57:21Z"}
```

**The ledger.** Once all had finished, `GET /ledger/verify` returned:

```json
{
  "verified": true,
  "chain_intact": true,
  "records": 15,
  "delivered_jobs": 5,
  "first_bad_record": null,
  "first_bad_job_id": null,
  "reason": null,
  "store_checked": 5,
  "store_mismatches": []
}
```

Five succeeded jobs, three records each; the failed job is not in the ledger. The
constellation job's delivered record:

```json
{
  "study_id": "e131f603894ea339d603b234c744b3c5ac0c0c5fdb29de03ea1e98d4b8fdfb94",
  "request_hash": "4a7db57cce2a739fd6a9ad6de5d0a97f0814b311b85fd0901689ba5837687b12",
  "run_hash": "4c19e283a5f5889ded7663c7941e66917d2ed432533fb9b65a7dc4f5bfa267d9",
  "delivered_utc": "2026-09-30T17:57:20Z",
  "prev_hash": "f65989b31ed743697c5a03265d6f0e9ee6c4ce70789654d727c06f09cd06244e",
  "record_hash": "be241d298ef2dd3956d620150f66bfcdc2d76cbe13e7d61a9a83211f2c877a99"
}
```

## Operations

`POST /jobs` takes `{"op": <name>, "request": <object>}`. Three operations ship:

| Operation | Request | Result | Artefacts | Entitlement |
|---|---|---|---|---|
| `run` | `{"scenario": "<TOML>"}`, any open or Pro kind | the run's result document, byte for byte | `chart.svg` and `table.csv` when the run gives them | the scenario kind's own tier (open kinds are free) |
| `dossier` | `{"scenario": "<TOML>", "issued_utc": "<stamp>"}` (stamp optional) | the study dossier JSON | `dossier.json`, `dossier.html` | the study dossier feature (`pro-study-dossier`), and the kind's tier |
| `ci` | `{"manifest": "<pack TOML>", "scenarios": {"<file>": "<TOML>", ...}, "baseline": <ci.json>, "threshold": <relative tolerance>, "generated_utc": "<stamp>"}` (last three optional) | the Pack C report JSON | `ci.json`, `report.html` | Pack C (`pro-scenario-ci`), and each scenario kind's tier |

The entitlement is checked when the job is submitted (403, nothing queued) and again
when it runs, since a licence can expire in between. In a `ci` request every manifest
entry must be a plain file name present in `scenarios`; the texts are written into the
job's `inputs/` directory and run from there.

With a data directory, `POST /study` answers with its original fields (`study_id`,
`pack`, `routes_to`, `status`) plus `job_id` and `job_state`, and queues a `dossier`
job for the request's `inputs.scenario`. That dossier has the same `result_hash` as
`kshana-pro study --dossier` on the same scenario (the acceptance test compares them).
A study request without `inputs.scenario` is acknowledged as before, with
`"job_id": null` and a note saying why no job was created. Without a data directory,
`POST /study` is unchanged, byte for byte.

## The job life cycle

```text
POST /jobs ──► queued ──► running ──► succeeded   (result.json, artefacts/, ledger records)
                  ▲          │
                  │          └──────► failed      (error text; never in the ledger)
                  └── restart: every queued or running job is queued again
```

- **Identity.** The job identifier is the SHA-256 of a fixed prefix, the operation name
  and the canonical JSON of the request (object keys sorted at every depth, no white
  space). The same request, with keys in any order, is the same job.
- **Storage.** `<data-dir>/jobs/<id>/` holds `request.json`, `status.json`,
  `result.json`, `artefacts/` and, for `ci`, `inputs/`. Every file is written to a
  temporary name, flushed and renamed into place. `status.json` is the commit point: a
  job is marked succeeded only after its result, its artefacts and its ledger records
  are on disk.
- **Attempts.** `attempts` counts how many times a worker started the job: 1 for a job
  that ran once; more only when a stop or a crash interrupted a run and the restarted
  server ran it again.

## What survives a restart

Everything on disk: every job, its state, its result, its artefacts and the ledger. On
start, every job in state `queued` or `running` is queued again in submission order; a
`running` job is one a crash interrupted, and it runs again from the start (the
operations are deterministic, so the rerun gives the same result). A finished job,
succeeded or failed, is never re-run; to run a request again, change it. A clean stop
lets each worker finish the job it is running; jobs still queued stay queued. The
acceptance test stops a one-worker server with jobs waiting, marks one as a crash would
leave it, starts a new server on the same directory, and requires every job to finish
with a result byte-identical to a direct run. Its control, with recovery switched off,
shows the same jobs left unfinished.

## Adding an operation

The operation table is `OPERATIONS` in `src/serve/ops.rs`. To add one:

1. Write `prepare(request, entitlement, now) -> Result<Prepared, OpError>`: parse the
   request; return `OpError::Invalid` for a malformed one (422) and
   `OpError::Forbidden` when the entitlement does not allow it (403). Keep it cheap: it
   runs on the connection thread.
2. Write `run(request, context) -> Result<OpOutput, String>`: return the result
   document (JSON text) and the artefacts as `(file name, text)` pairs. Names must be
   plain file names. Reach the engine only through `crate::registry::run_gated` or an
   overlay function built on it.
3. Add one `Operation { name, summary, feature, prepare, run }` entry to
   `OPERATIONS`.

The routes, the store, the workers, the ledger and the OpenAPI document pick the entry
up from the table; nothing else changes. A new Pro scenario kind needs no new operation:
it runs through `run` as soon as it is registered.

## How the ledger is verified

Each succeeded job appends three records to `<data-dir>/ledger.json` with the overlay's
hash-chained study ledger: request (`request_hash` = SHA-256 of `request.json`), run
(adds `run_hash`) and delivered (adds `delivered_utc`, the server clock). `run_hash` is
the SHA-256 of the delivery manifest, the text

```text
<sha256 of result.json>  result.json
<sha256 of the artefact>  artefacts/<name>        (one line per artefact, in name order)
```

so it covers the result and every artefact. Each record's `record_hash` is the SHA-256
of the previous record's hash followed by the record's canonical content, and its
`prev_hash` must equal the previous `record_hash`. `GET /ledger/verify`:

1. recomputes the chain; if a record no longer matches, it reports `verified: false`,
   `chain_intact: false`, and names the first such record (`first_bad_record`, counted
   from 0, and the job identifier it carries). The acceptance test changes one
   hexadecimal digit of one record's request hash in the file and requires that record
   to be named;
2. for every delivered record, re-reads `request.json`, `result.json` and each artefact
   from the store and recomputes both hashes; a file changed after delivery is listed
   in `store_mismatches` and `verified` is false.

The ledger is never extended while it fails verification, so a break cannot be buried
under new records. A job is appended at most once, so a job that is re-run after a crash
between the ledger write and the status write is not recorded twice.

## The job board

`GET /` is a self-contained page (inline style and script, no external asset, a
Content-Security-Policy that allows connections only to the server itself). It embeds
the current job list when served, so it shows real jobs at once, and refreshes from
`GET /jobs` every 5 seconds. For each job it shows the operation, the kind, the state
(with the error text of a failed job), the evidence tier the result itself states (and
says so when it states none), the server-clock
times, the attempts and links to the result and the artefacts.

## Limitations

- **One request per connection.** Every response closes the connection; there is no
  keep-alive. A client polls `GET /jobs/<id>` or follows `GET /jobs/<id>/events`.
- **One entitlement for the whole process.** Every caller is served at the tier of the
  process's licence; there is no per-user tier.
- **Keys and tokens, not a user directory.** Callers are named API keys (`--keys-file`) or
  OIDC tokens from the site's identity provider (`--oidc`), each with a role and a tenant;
  see "Access control and audit" below. The single `--api-token-file` token still works
  and acts as an admin key. Users, groups and expiry live in the identity provider.
- **TLS needs the operator's key pair.** `--tls-cert`/`--tls-key` take PEM files (the
  chain and its private key); the server neither requests nor renews them. Without them it speaks plain
  HTTP for a reverse proxy in front. A browser cannot add the bearer header to a page
  load, so the job board needs the proxy to add it when a token is set.
- **A job store is not a backup.** Use `kshana-pro backup create` (below) on a schedule.
- **Cancelling abandons the run, it does not interrupt the engine.** A cancelled or
  timed-out job is finished at once and its output is never stored, but the engine
  computation it started keeps its thread until it ends on its own. Many cancelled
  long runs at once therefore still use processor time until they finish.
- **High availability is one active node.** Several nodes serve the API; one runs jobs.
  See "High availability" for what the shared storage must provide and the pause case.
- **No retention.** Jobs are kept until an operator deletes their directories; deleting
  a delivered job's directory makes `GET /ledger/verify` report it as a mismatch.

## Hardening (roadmap F2a)

| What | How |
|---|---|
| TLS in the binary | `serve --tls-cert <chain.pem> --tls-key <key.pem>`: TLS 1.3 and 1.2, forward-secret suites only (rustls with the ring provider) |
| Streaming | `GET /jobs/<id>/events`: server-sent events, one `status` event per change until the job finishes (at most one hour; a comment line every 15 s keeps proxies open) |
| Cancellation | `POST /jobs/<id>/cancel` (submitter): a queued job is `cancelled` at once; a running one stops within 50 ms; a finished one answers 409 |
| Time limit | `"timeout_secs": n` in the `POST /jobs` body (1 s to one week): past it the job is `failed` with "timed out" |
| Priority | `"priority": "high" \| "normal" \| "low"` in the `POST /jobs` body: three first-in, first-out lanes, high served first. Neither option changes the job's identifier |
| High availability | `serve --ha-node <name> [--ha-lease-secs <s>]` on two or more machines with one shared `--data-dir` (Enterprise, `pro-service-ha`) |
| Backup | `kshana-pro backup create --data-dir <dir> --out <new dir>` while serving; `kshana-pro backup verify <dir>` offline |

### High availability

Every node serves the API; the node holding the lease (`<data-dir>/ha/lease.json`)
runs jobs. It renews the lease every third of the lease time (default lease 30 s). A
standby that finds the lease expired takes it, queues every job left queued or running
(the failed node's running job runs again, with `attempts` 2), and runs the queue from
then on. A job submitted through a standby is stored on the shared disk and run by the
holder within a third of the lease time. A clean stop gives the lease up at once. Each
node keeps its own audit log, `audit/<node>/`. `kshana_runs_jobs` in `/metrics` says
which node is active.

The shared directory must provide exclusive file creation and atomic rename (a local
disk, NFS version 3 or later, SMB). A holder that is paused for longer than the lease,
for example cut off from the shared storage, may finish the job it was running after a
standby has started the same job again; choose a lease well above any expected pause.

### Backup and restore

`backup create` copies the ledger first, then the jobs and the audit logs, so every
delivery in the copied ledger finds its files; it writes `backup-manifest.json` with
every file's SHA-256. `backup verify` recomputes each hash and runs the ledger check on
the copy. To restore, stop the service, put the backup in place of the data directory,
delete `backup-manifest.json`, and start the service: jobs copied while running are
run again.

## The hosted trial sandbox (roadmap F3b)

`kshana-pro serve --trial --data-dir <dir> [--trial-jobs-per-day <n>]
[--trial-max-run-secs <s>]` opens the job service to anyone, with no credential, for a
public trial:

- **Evaluation tier**, whatever licence the server holds: open scenario kinds and the
  free seam check only.
- **One tenant per caller**: `trial-` and 16 hexadecimal characters of a salted SHA-256
  of the caller's address (salt in `<data-dir>/trial-salt`). A caller sees only its own
  jobs; no address is stored or shown.
- **Quotas**: 20 new jobs a day per caller (counted from the job store, so a restart
  does not reset them; resubmitting the same request is free), a time limit of at most
  60 s per job, 120 requests a minute per address, 100 trial jobs waiting server-wide.
  Past a quota the answer is 429 (503 when the queue is full) with `Retry-After`.
- **Closed**: `POST /run` (use `POST /jobs`), `POST /study`, and every admin route.
- A trial server takes no access token, keys or OIDC; it refuses to start with them.

Run it behind TLS (`--tls-cert`/`--tls-key`) on its own data directory. Behind a reverse
proxy every caller has the proxy's address, so run it directly, not behind a proxy.
Hosting it publicly needs a machine and a domain from the owner.

## Service-level metrics (roadmap F2b)

`GET /metrics` (reader) gives Prometheus text for the caller's tenant (every tenant for
an operator): `kshana_jobs{tenant,state}`, `kshana_job_queue_wait_seconds` and
`kshana_job_run_seconds` (median, 95th and 99th percentile, sum, count, per tenant,
over finished jobs), `kshana_http_requests_total{tenant,route,status}`,
`kshana_queue_length`, `kshana_uptime_seconds`, `kshana_runs_jobs`. Job figures come
from the store; request counts are this process's since it started.

## Access control and audit

Design: `docs/design/A9-INTEGRATION-SECURITY.md`.

| Role | May call |
|---|---|
| `reader` | every GET route: version, kinds, OpenAPI, the job board, job lists, status, results, artefacts |
| `submitter` | also `POST /run`, `POST /study`, `POST /jobs` |
| `admin` | also `/ledger`, `/ledger/verify` and `/audit/verify`, but only with tenant `*` (or `default`): these span every tenant |

**API keys.** `kshana-pro keys add --file keys.toml --id site-a-ingest --tenant site-a --role submitter`
prints a new key once. The file keeps only its SHA-256. `keys revoke` and `keys list` manage the file;
the server reads it at start (`--keys-file keys.toml`). A key of tenant `*` must be an admin; it sees
every tenant's jobs.

**OIDC.** `--oidc oidc.toml` accepts JWTs from the site's identity provider, verified offline against
a JWKS file next to the config (replace the file to rotate keys; it is re-read when it changes):

```toml
issuer = "https://idp.example/realms/ops"
audience = "kshana-pro"
jwks_file = "jwks.json"
role_claim = "kshana_role"     # "reader" | "submitter" | "admin", or an array (the highest wins)
tenant_claim = "kshana_tenant"
# id_claim = "sub"; leeway_s = 60
```

ES256 and RS256 (a modulus of 2048 bits or more) are accepted; `none` and the HMAC algorithms are
refused. A token without the role or tenant claim is refused, and an OIDC token never gets tenant `*`.

**Tenants.** A job belongs to the tenant that submitted it. Another tenant's job answers 404 on every
route, exactly as a job that does not exist, and never appears in a list or on the board. The same
request from two tenants makes two jobs. Jobs stored before tenants existed belong to `default`.

**Audit log.** With a data directory, every request except `GET /health` adds one line to
`<data-dir>/audit/YYYY-MM-DD.jsonl` (UTC): time, caller id, tenant, role, source, method, route template,
job id, status and duration. A key, a token, a body, a query string or a client address is never written.
Each line holds the SHA-256 of the line before, so `GET /audit/verify` names the first edited or
removed line. Day files older than `--audit-retention-days` (default 400) are deleted at start, and each
deletion is logged. If the log cannot be written, the request is answered 503 rather than served
unrecorded.

## Clients

`clients/python/kshana_pro_client.py` (Python 3.9+, standard library only) and
`clients/typescript/kshana_pro_client.ts` (anything with `fetch`: Node 18+, Deno, browsers) are generated
from the route table by `kshana-pro clients --out clients`, so every route has a method. Method names
are the OpenAPI operation ids: `post_jobs`, `get_jobs_id_result` in Python, and `postJobs`,
`getJobsIdResult` in TypeScript. Both send the API key or OIDC token as a bearer credential, raise
`KshanaProError` (status and decoded body) on anything other than 2xx, and add `wait_job` / `waitJob`.

```python
from kshana_pro_client import KshanaProClient
c = KshanaProClient("http://127.0.0.1:8088", token=os.environ["KSHANA_PRO_KEY"])
job = c.post_jobs({"op": "run", "request": {"scenario": open("scenario.toml").read()}})
c.wait_job(job["job_id"])
result = c.get_jobs_id_result(job["job_id"])
```

`tests/service_clients.rs` checks that the committed clients are what the generator makes today, and
drives both against a running server. Each one submits a job, waits for it, reads the result and an
artefact, and gets 403 for a reader's submission and 401 without a key.

