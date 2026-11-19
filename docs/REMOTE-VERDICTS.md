# Remote verdicts

Owner decision, 19 November 2026. Some methods ship in **no** Pro build. They run only on
Ashforde's hosted verdict service, so the method never reaches a customer machine and cannot
be extracted. A licensed customer still gets the result:

```
kshana-pro verdict survey request.json --url https://<service> --out verdict.json
kshana-pro verdict verify verdict.json
```

1. The client sends the method's inputs with the customer's licence token (from `--license`
   or the token activated on this machine). The token goes to `curl` on stdin, never on the
   command line.
2. The service checks the licence (valid, unexpired, not revoked, Pro tier or above), applies
   per-licence limits, runs the method and returns a **signed verdict**: the outcome (numbers
   rounded), its evidence class, and an Ed25519 signature. Intermediate values, traces,
   tuning and full grids are never returned (the full survey grid only to Enterprise).
3. The client accepts the answer only if the signature verifies against a trusted service
   key: `--trust`, else `KSHANA_VERDICT_KEY` at run time, else the key built into the
   release (`KSHANA_VERDICT_KEY` at build time). Several keys may be given, comma separated,
   during a key rotation. A saved verdict can be re-verified any time, offline.

Methods served: `selfcal` (self-calibrating simulator loop), `survey` (integrity-driven ASF
survey planner). Endpoint: `--url` or `KSHANA_VERDICT_URL`; HTTPS only (a loopback `http://`
address is allowed for a local test service).

The client (`src/remote_verdict.rs`) holds no method code and names no service. The service
lives in a separate private repository and is never linked into this crate (X3); its
identifiers are registered `[core]` terms, so the source guard and the binary check fail if
one ever appears here.

Customers who cannot reach a network use the hardened on-prem build instead, and only under
a signed on-premises contract: the licence must carry the explicit `onprem-contract` grant as
well as being Enterprise and machine-bound (`docs/OFFLINE-PROTECTION.md`).
