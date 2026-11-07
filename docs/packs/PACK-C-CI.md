<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->

# Pack C in your CI: the subscriber guide

Roadmap F5. How a Pack C subscription reaches a build pipeline, from the purchase to a
failing build on a resilience regression. What Pack C checks is in
[PACK-C.md](PACK-C.md).

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| CI | continuous integration |
| HTML | HyperText Markup Language |
| JSON | JavaScript Object Notation |
| URL | uniform resource locator |

## 1. The subscription

* **Edition:** `pack-c-ci-monthly` (`commercial/payments.toml`): Pro tier, product
  `cli`, 31 days per paid month. The price is set by the owner; until payments are
  switched on, subscriptions are arranged by e-mail and the same token is issued by
  hand from the register (`kshana-pro license issue ... --register`).
* **Token:** sent by e-mail after the purchase (docs/PAYMENTS.md). Each monthly payment
  renews it (a new token, 31 days on); a cancelled subscription's token is revoked when
  the subscription ends; a refund revokes it at once. A CI runner holds no licence store,
  so the token is passed with `--license` from a secret; update the secret when the
  renewal e-mail arrives.

## 2. The pipeline

1. Put the token in a masked secret, `KSHANA_PRO_LICENSE`.
2. Make the Kshana Pro binary available to the runner (your artefact store, or vendored).
3. Commit your pack (`kshana/pack.toml` and its scenarios) to the repository.
4. Add the job: [GitHub Actions](../../examples/ci/github-actions-pack-c.yml) or
   [GitLab CI](../../examples/ci/gitlab-ci-pack-c.yml).

Each run: `kshana-pro ci --pack kshana/pack.toml [--baseline baseline.json] --license
"$KSHANA_PRO_LICENSE" --json ci.json --out report.html`. Exit 0 on pass (or the first,
informational run with no baseline), 1 on a regression, 2 on a usage error. A green run
on the default branch becomes the next baseline.

## 3. When the token stops

| What happened | What the pipeline shows |
|---|---|
| The renewal is due in 30 days or less | A `[licence] Renew soon` (7 days: `Renew now`) line in the log; the build still runs |
| The token expired less than 14 days ago | A grace warning; the build still runs |
| Expired 14 days ago or more, or revoked | `error: license invalid`, exit code 1, no report |

A revocation reaches a runner only through a revocation list installed on it (the
runner image's licence store); a runner with no list runs a revoked token until it
expires. Keep the token's expiry short (31 days) for that reason.

## 4. Proof

`tests/pack_c_subscription.rs` runs the whole flow in test mode: a signed purchase
webhook issues the token, the `ci` command runs the `resilience-baseline` pack with it
twice (first informational, then gated against the first), a renewal webhook renews the
token, and after a refund webhook and the revocation list it produces, the same command
is refused.
