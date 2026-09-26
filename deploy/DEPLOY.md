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

`POST /run` is **fail-closed**: open scenarios are always free; Pro scenarios require
the serving entitlement's tier (set by `--license`). Denied → HTTP 403; invalid
scenario → 422; oversized request → 413.

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

## Smoke test

```bash
curl -s localhost:8088/health                      # ok
curl -s localhost:8088/version
curl -s -X POST --data-binary @scenario.toml localhost:8088/run
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
  throughput, run multiple replicas behind a load balancer.
- **Limits:** requests are capped (header 16 KiB, body 1 MiB) as a basic DoS guard.
