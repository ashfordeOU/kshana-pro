# Remote verdicts

Owner decision, 19 November 2026. Some methods ship in **no** Pro build. They run only on
Ashforde's hosted verdict service, so the method never reaches a customer machine and cannot
be extracted. A licensed customer still gets the result:

```
kshana-pro verdict survey request.json --url https://<service> --trust <service public key> --out verdict.json
kshana-pro verdict verify verdict.json --trust <service public key>
```

1. The client sends the method's inputs with the customer's licence token (from `--license`
   or the token activated on this machine). The token goes to `curl` on stdin, never on the
   command line.
2. The service checks the licence (valid, unexpired, `pro-eloran`-entitled), runs the method
   and returns a **signed verdict**: the outcome, its evidence class, and an Ed25519
   signature. Intermediate values, traces, tuning and full grids are never returned.
3. The client accepts the answer only if the signature verifies against the pinned service
   key (`--trust` / `KSHANA_VERDICT_KEY`). A saved verdict can be re-verified any time, offline.

Methods served: `selfcal` (self-calibrating simulator loop), `survey` (integrity-driven ASF
survey planner). Endpoint: `--url` or `KSHANA_VERDICT_URL`; HTTPS only (a loopback `http://`
address is allowed for a local test service).

The client (`src/remote_verdict.rs`) holds no method code and names no service. The service
lives in a separate private repository and is never linked into this crate (X3); its
identifiers are registered `[core]` terms, so the source guard and the binary check fail if
one ever appears here.

Customers who cannot reach a network use the hardened on-prem build instead
(`docs/OFFLINE-PROTECTION.md`).
