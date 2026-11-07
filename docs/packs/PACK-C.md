<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->

# Pack C — Scenario-Subscription CI

**Catch PNT-resilience regressions in CI — before they ship in your firmware or config.**

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

A subscription to versioned, MODELLED resilience scenario packs that you run against
your own receiver config / model in CI on every build. Each run scores your figures of
merit, compares them to your last green baseline, and **fails the build on a
regression** — with a branded, reproducible study report attached. Built on the open
[Kshana](https://github.com/AshfordeOU/kshana) PNT engine; delivered by
[Ashforde OÜ](https://ashforde.org).

---

## Who it's for

Teams that ship PNT firmware, configs, or models and don't want a silent resilience
regression to reach the field:
- GNSS **receiver and chipset vendors** (algorithm + integration groups),
- **integrators** building navigation boxes on top of a receiver,
- anyone whose build produces a PNT config/model whose resilience can drift between
  versions.

## The job it does

You changed a tracking loop, an integrity setting, a detector threshold, or a fusion
weight. It still passes your functional tests — but did it quietly cost you
availability, protection-level margin, or detection power under interference? Pack C
turns that question into a **CI gate**: it re-runs a fixed, versioned set of resilience
scenarios against your build, compares the figures of merit to your baseline, and breaks
the build if a known-direction metric degraded beyond your tolerance. No more finding
out in integration — or in the field.

## How the subscription works

1. **You subscribe to a versioned scenario pack** — a stable `id` + `version` (e.g.
   `resilience-baseline` v1.0.0) naming an ordered, labelled set of resilience
   scenarios. The pack carries a content hash so a report can pin exactly which pack
   contents produced it.
2. **You run it in CI each build** — point the runner at your scenario config(s) and,
   optionally, the `ci.json` from your last green build as a baseline.
3. **The gate decides** — every figure of merit with a known better/worse direction is
   compared against the baseline under your tolerance (default 5%). The build passes iff
   there are no regressions.

## What you send (intake)

- **Your scenario config(s)** — the resilience scenario(s) parameterised for your
  receiver/model (the same TOML the open engine consumes).
- **Optionally, a prior baseline `ci.json`** — the machine-readable result from your
  last green run. With no baseline, the run is *informational* (scored, but nothing is
  gated).

## What you get (deliverable)

- A branded, MODELLED **PNT-Resilience Study Report** (HTML) — the CI verdict as the
  headline (informational / PASS / a count of REGRESSIONS), the full sorted figures of
  merit (each regressed metric annotated with its baseline → current Δ%), a regressions
  sub-table, and an audit-grade provenance block.
- A **machine-readable CI result** (`ci.json`) — the scored report you store as the next
  baseline.
- A **CI exit code** — `0` on pass, `1` on regression — so it drops straight into any
  pipeline.

Sample: [`sample-scenario-ci-report.html`](sample-scenario-ci-report.html) — generated
from the versioned `resilience-baseline` sample pack with **no customer data**, against
an illustrative baseline that shows one regression and one improvement.

## What it is — and is not

- It **is** decision-support that feeds *your* verification & validation: a reproducible,
  baseline-relative check that flags resilience drift between builds.
- It is **MODELLED**, and it is **not a certification**, not a guarantee of field
  performance, and not ESA acceptance or TRL maturation. The scenarios are synthetic /
  parameter-grounded, not flight or in-orbit data.
- The gate is **honest by construction**: only figures of merit with a *known* better/
  worse direction are gated. A metric whose direction the gate cannot infer (or a
  near-zero baseline) is reported as **informational** — surfaced for a human, never
  silently turned into a pass or a fail.

## Reproducibility

Every run pins what produced it: the **pack id + version + content hash**, the **engine
version**, and the **per-scenario hashes**. The library reads no clock and no randomness
(timestamps are caller-supplied; scenarios are seeded), so a report is byte-identical
across regenerations from the same inputs.

## Start a subscription

Running it in your pipeline (token, GitHub Actions and GitLab CI jobs, renewals):
[PACK-C-CI.md](PACK-C-CI.md).

Contact **[contact@ashforde.org](mailto:contact@ashforde.org)** · [ashforde.org](https://ashforde.org).
Pack C is delivered under a commercial engagement (Pro tier). Pack scope, cadence, and
pricing are set per engagement — see the rate card (figures provided on request).

---

*Kshana is a performance simulator for studies and trade-offs, not a navigation product.
"Kshana" and associated marks are trademarks of Ashforde OÜ.*
