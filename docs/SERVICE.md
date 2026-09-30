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
| CI | continuous integration (here: Pack C, the scenario-subscription CI) |
| CSV | comma-separated values |
| DOP | dilution of precision (PDOP: position DOP) |
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
| `--data-dir <dir>` | Turns the job service on. The job store, the artefacts and the ledger live here. Without it the server answers exactly the routes it answered before, with the same behaviour. |
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
"..."), and `evidence_tier` is its leading word: VALIDATED, MODELLED or PARTNER.

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
(with the error text of a failed job), the engine's evidence tier, the server-clock
times, the attempts and links to the result and the artefacts.

## Limitations

- **One request per connection.** Every response closes the connection; there is no
  keep-alive and no streaming. A client polls `GET /jobs/<id>`.
- **One entitlement for the whole process.** Every caller is served at the tier of the
  process's licence; there is no per-user tier.
- **A bearer token is not user management.** It is one shared secret for every caller,
  with no users, roles, expiry or audit of who called. Put per-user authentication in
  the site's own gateway.
- **No transport encryption inside the binary.** The server speaks plain HTTP. Put it
  behind the site's own reverse proxy for TLS. A browser cannot add the bearer header to
  a page load, so the job board needs the proxy to add it when a token is set.
- **A job store is not a backup.** The data directory is the only copy of the results
  and the ledger; back it up (`deploy/DEPLOY.md` shows how).
- **One server per data directory.** Two processes on the same directory would each run
  the queued jobs; nothing locks the directory.
- **No cancellation and no time limit.** A running job runs to completion; a stop waits
  for it. There is no priority between queued jobs: first in, first out.
- **No retention.** Jobs are kept until an operator deletes their directories; deleting
  a delivered job's directory makes `GET /ledger/verify` report it as a mismatch.
