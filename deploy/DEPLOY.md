# Kshana Pro — Deployment (PR22)

How to deploy the Kshana Pro service on-prem, hosted, or air-gapped/sovereign.

> Proprietary. Not for the public repo.

---

## What gets deployed

`kshana-pro serve` runs a **dependency-light, std-only HTTP/1.1 service** (no async
runtime) over the overlay. It makes **no external calls and emits no telemetry** —
the entire request path is auditable, which is the right posture for an air-gapped
or sovereign install.

**Routes**

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness probe (`ok`) |
| GET | `/version` | Overlay + embedded-engine info |
| GET | `/kinds` | Open + Pro scenario kinds (with min tier) |
| POST | `/run` | Body = scenario TOML → result JSON (entitlement-gated) |
| POST | `/study` | Validate + acknowledge a study request; with a data directory, also queue its dossier job (`job_id` in the reply) |
| GET | `/openapi.json` | OpenAPI 3.1 document, generated from the router's own route table |

`POST /run` is **fail-closed**: open scenarios are always free; Pro scenarios require
the serving entitlement's tier (set by `--license`). Denied → HTTP 403; invalid
scenario → 422; oversized request → 413.

**Job service routes** (only when the server is started with `--data-dir <dir>`):

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/` | Job board: a self-contained page (no external assets) listing the jobs |
| POST | `/jobs` | Submit `{"op": ..., "request": ...}` → 202 with the job identifier (200 if that exact job already exists; 403 when the entitlement does not allow it, nothing queued) |
| GET | `/jobs` | List jobs, newest first (`?limit=`, `?state=`) |
| GET | `/jobs/<id>` | Status: state, server-clock times, attempts, error text |
| GET | `/jobs/<id>/result` | Result document (409 while unfinished or when failed) |
| GET | `/jobs/<id>/artefacts/<name>` | One stored artefact (chart, table, dossier page, CI report) |
| GET | `/ledger` | The hash-chained delivery ledger |
| GET | `/ledger/verify` | Verify the chain and every delivered job's stored files |

Operations: `run` (any open or Pro kind), `dossier` (the study dossier) and `ci` (a
Pack C scenario pack with its scenario texts inline). The full guide, with a recorded
session, is `docs/SERVICE.md`.

## Image

Pro depends on the open core by path, so build from the **parent** directory holding
both `kshana/` and `kshana-pro/`:

```bash
docker build -f kshana-pro/Dockerfile -t kshana-pro:latest ..
```

The image is a minimal `debian:bookworm-slim` runtime, **non-root** (uid 10001),
exposing `8088`.

## Standard / on-prem / hosted

```bash
export KSHANA_PRO_LICENSE_PUBKEY="<licence public key, 64 hex characters>"
docker compose -f kshana-pro/deploy/docker-compose.product.yml up --build
```

Runs fail-closed at **Evaluation** tier (open engine + free seam-check). To unlock
Pro features, issue a token on the vendor side and pass it as the command's
`--license` (see the commented `command:` line in the compose file):

```bash
# Vendor side only, never on the customer's machine:
KSHANA_PRO_SIGNING_KEY=... kshana-pro license issue --tier pro --licensee "Customer" --days 365   # → token
```

Tokens are Ed25519-signed. The image contains only the **public** key, baked in at
build time, so a customer holding the image can check tokens but cannot mint them.
A release build also refuses `--dev` and contains no demo key.

## Air-gapped / sovereign

```bash
# On a connected machine:
docker build -f kshana-pro/Dockerfile -t kshana-pro:latest ..
docker save kshana-pro:latest -o kshana-pro.tar
# Move kshana-pro.tar into the enclave, then:
docker load -i kshana-pro.tar
docker compose -f kshana-pro/deploy/docker-compose.sovereign.yml up
```

The sovereign compose is hardened: read-only rootfs, `no-new-privileges`, all Linux
capabilities dropped, tmpfs scratch, bounded CPU/memory, loopback-bound port. The
service needs **no outbound connectivity** — block egress at your enclave's network
policy; nothing in Kshana attempts it.

## The data directory (job service)

`--data-dir <dir>` turns the job service on. The compose files mount a named volume
`kshana-pro-data` at `/var/lib/kshana-pro` (the image creates that directory owned by
the non-root user), so the root file system stays read-only. Layout:

```text
<data-dir>/
  jobs/<job_id>/request.json     the submitted request (written once)
  jobs/<job_id>/status.json      state, server-clock times, attempts, error text
  jobs/<job_id>/result.json      the result document (succeeded jobs)
  jobs/<job_id>/artefacts/       named artefacts (succeeded jobs)
  jobs/<job_id>/inputs/          input files an operation needed on disk (ci)
  ledger.json                    the hash-chained delivery ledger
```

`--workers <n>` sets the worker threads (1 to 64, default 1). Every write is a
temporary file renamed into place, so a crash leaves no half-written file. On start,
every job left `queued` or `running` is queued again; a finished job is never re-run.
Run **one server per data directory**: two processes on the same directory would each
run the queued jobs.

**Backup.** The job store is not a backup. Copy the whole directory while the server is
stopped (or at least while no job is running), for example:

```bash
docker compose -f kshana-pro/deploy/docker-compose.product.yml stop
docker run --rm -v kshana-pro-data:/data -v "$PWD":/backup debian:bookworm-slim \
  tar -C /data -czf /backup/kshana-pro-data-$(date -u +%Y%m%dT%H%M%SZ).tgz .
docker compose -f kshana-pro/deploy/docker-compose.product.yml start
```

**Restore.** Stop the service, empty the volume, unpack the archive into it, start the
service, then check `GET /ledger/verify` returns `"verified": true`:

```bash
docker compose -f kshana-pro/deploy/docker-compose.product.yml stop
docker run --rm -v kshana-pro-data:/data -v "$PWD":/backup debian:bookworm-slim \
  sh -c 'find /data -mindepth 1 -delete && tar -C /data -xzf /backup/<archive>.tgz && chown -R 10001 /data'
docker compose -f kshana-pro/deploy/docker-compose.product.yml start
curl -s localhost:8088/ledger/verify
```

## Access token

`--api-token-file <path>` makes every route except `/health` require
`Authorization: Bearer <token>`; without it, the behaviour is as before. The token is
read from the file (one line; surrounding white space is ignored), only its SHA-256 is
kept in memory, the comparison runs in constant time, and it is never logged or echoed.
In the compose files, mount it as a secret (commented lines show how) and pass
`--api-token-file /run/secrets/kshana_api_token`. A bearer token is one shared secret,
not user management: put per-user authentication, and transport encryption, in your
gateway. A browser cannot add the header to a page load, so to use the job board with a
token, let the gateway add the header for authenticated users.

## Smoke test

```bash
curl -s localhost:8088/health                      # ok
curl -s localhost:8088/version
curl -s -X POST --data-binary @scenario.toml localhost:8088/run
# With --data-dir:
python3 -c 'import json,sys; print(json.dumps({"op":"run","request":{"scenario":open(sys.argv[1]).read()}}))' scenario.toml \
  | curl -s -X POST --data-binary @- localhost:8088/jobs
curl -s localhost:8088/jobs
curl -s localhost:8088/ledger/verify
```

## Operational notes

- **Licensing in production:** create the vendor key pair once with
  `kshana-pro license keygen`. Keep `KSHANA_PRO_SIGNING_KEY` in the vendor's secret
  store; it never goes into a build, an image or a customer's hands. Bake
  `KSHANA_PRO_LICENSE_PUBKEY` into customer builds. Re-keying invalidates every
  token issued under the old key, so customers need new tokens and a new build.
- **Not covered:** tokens are not bound to a machine, so a token can be copied to a
  second installation until it expires; keep terms short for binaries you hand over.
- **TLS / auth:** terminate TLS and add authentication at your gateway/ingress; the
  service speaks plain HTTP and assumes a trusted front end.
- **Scale:** the server is one-thread-per-connection (Connection: close). For high
  throughput, run multiple replicas behind a load balancer; with the job service, give
  each replica its own data directory (a job's identifier is only known to the replica
  that stored it, so route `/jobs/<id>` requests back to that replica).
- **Limits:** requests are capped (header 16 KiB, body 1 MiB; 16 MiB for `POST /jobs`)
  as a basic denial-of-service guard. A body above the cap is refused with 413 before
  it is read.

## Linux package and the air-gapped bundle (roadmap L6, L7)

**Debian package.** `scripts/build-deb.sh <kshana-pro binary> <out-dir> [arch]` builds
`kshana-pro_<version>_<arch>.deb`: the binary in `/usr/bin`, the systemd unit
`kshana-pro.service` (local-only by default, hardened: a `kshana` system user, read-only
system, only `/var/lib/kshana-pro` writable), its settings in `/etc/kshana-pro/serve.conf`
(a configuration file, kept on upgrade) and the guides. The post-install step creates the
user and the directories and says how to activate the licence and start the service. The
package is unsigned; a signed apt repository is the publisher's step.

**RPM package.** `scripts/build-rpm.sh <kshana-pro binary> <out-dir> [arch]` (needs
`rpmbuild`; `arch` is `x86_64` or `aarch64`) builds `kshana-pro-<version>-1.<arch>.rpm`
with the same files: the binary, the unit in `/usr/lib/systemd/system`, the settings as a
`%config(noreplace)` file, the guides and the licence text. It creates the `kshana` system
user before the files land, owns `/var/lib/kshana-pro` and its `data/` and `licence/`
directories (mode 0750), stops the service on removal and restarts it on upgrade. A
pre-release version (`0.41.0-rc.1`) becomes `0.41.0~rc.1`, which RPM orders before
`0.41.0`. The package is unsigned; a signed dnf or zypper repository is the publisher's
step.

**Air-gapped bundle.** `scripts/offline-bundle.sh <kshana-pro binary> <out-dir> [--sign
<key file> --signer <name>] [--revocations <revocations.txt>] [--deb <package>]` writes a
directory to carry into a site with no network: the binary, the unit and its settings, the
guides, the signed revocation list, a CycloneDX bill of materials of the locked build graph
(`sbom.cdx.json`), `SHA256SUMS`, and with `--sign` an Ed25519 signature over every file
(`signature.json`, made by `kshana-pro dossier sign`; the signing licence comes from
`$KSHANA_PRO_VENDOR_LICENSE`). On the site:

```bash
sha256sum -c SHA256SUMS                                   # or: shasum -a 256 -c SHA256SUMS
kshana-pro dossier verify . --trust <publisher's bundle public key>   # or scripts/verify-dossier-signature.py
./install.sh --prefix /opt/kshana --licence token.txt [--service]
```

`install.sh` checks every file before installing anything, installs the revocation list
when the bundle carries one, activates the licence offline, and with `--service` installs
the systemd unit. Without `--sign` the bundle says it is unsigned (`BUNDLE.txt`); the
publisher's bundle signing key is not yet made (docs/LICENSING-KEYS.md applies to it).
