# Ground software deployment integrity and configuration drift (evidence only)

Module `src/deploy_attest.rs`, command `kshana-pro deploy-attest`
(`src/cli/deploy_attest_cmd.rs`), tests in `tests/custody.rs` (module `deploy_attest`). Feature
`pro-deploy-attestation` (Pro).

**What already existed.** The signed pack (`src/signed_dossier.rs`) hashes and signs any
directory. Chain of custody (`src/custody`) seals captures and their handovers. Nothing compared
deployed ground software with a released baseline over time. This command builds that on the
signed pack.

**Scope: evidence only.** Nothing here connects to a host, scans a network, blocks, repairs or
detects intrusions. It compares hash lists the operator supplies from their own tooling
(`sha256sum` output, or CSV `path,sha256`).

## Baseline

`kshana-pro deploy-attest baseline <release-dir|hashes.sha256> --deployment <name> --release <id>
--out <dir> --signer <name> --key-file <file> [--exclude 'logs/*,*.pid']` hashes every file of the
released deployment. A symbolic link is recorded by its target, not followed. The command writes
`manifest.json` (`kshana-pro-deploy-baseline/1`) and `baseline.sha256`, and signs the pack.

## Check

`request.toml`:

```toml
title = "Ground segment: deployment drift, November"
baseline = "baseline-pack"          # the signed baseline pack
exclude = ["logs/*"]                # added to the baseline's own exclusions

[[snapshot]]
host = "gs-a"
utc = "2026-11-08T06:00:00Z"
file = "gs-a-2026-11-08.sha256"

[[accepted]]                        # a change accepted by a stated reference
path = "etc/stations.conf"
sha256 = "<hex>"
reference = "CR-0042"
# host = "gs-a"                     # optional: only on this host
```

`kshana-pro deploy-attest check request.toml --trust <key hex> --out <dir> --signer <name>
--key-file <file>`:

* first verifies the baseline pack against the trusted key. A missing key, a different key or
  a changed file stops the check;
* compares each snapshot with the baseline and sorts every file into matched, **modified**,
  **missing**, **added** or **accepted** (it matches an `[[accepted]]` entry: path, hash and,
  if given, host);
* builds per host a **drift timeline** of the snapshots in time order: the files that newly
  left the baseline and the files that came back. It also lists **drift episodes**: per file
  and host, first seen, reverted (or open), and duration in days;
* writes and signs `report.json` (`kshana-pro-deploy-attestation/1`), `report.html`, the
  request and every input with its hash.

`kshana-pro deploy-attest synth <dir>` writes a SYNTHETIC release and six snapshots of two hosts.
The snapshots include a modified configuration, an added script, a missing library, an accepted
change and excluded log churn.

MODELLED: a comparison of the supplied hash lists. A change made and reverted between two
snapshots is not seen. Whether a deployment or a change is acceptable is for its operator to
decide.
