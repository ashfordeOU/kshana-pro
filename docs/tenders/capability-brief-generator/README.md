<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NEVER publish in the open kshana repo.
Index + usage for the per-prime tailored capability-brief generator (dominance-plan step A13).
OUTREACH PARKED until the founder explicitly says go. Contact: contact@ashforde.org
-->

# Per-prime capability-brief generator (PRIVATE — outreach PARKED)

**PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; never publish in the open kshana repo.**
**Contact: contact@ashforde.org**

This folder is the **asset on the shelf** for dominance-plan step **A13**: a parameterised 1–2pp capability-brief template plus three pre-rendered prime-archetype briefs. It lets us produce a tailored, prime-specific leave-behind in minutes **the moment the founder unparks outreach** — without contacting anyone before then.

## The PARKED-outreach rule (non-negotiable)

> **Producing a brief is a drafting step. Sending it is a separate, founder-gated step.**
>
> - The actual **prime/partner SELECTION** is **PARKED** until the founder explicitly says go.
> - **No real candidate name** is populated anywhere in this folder — only the three **archetypes**.
> - **No outreach, no contact, no email, no LinkedIn, no introductions** until founder go.
> - When the founder says go: bind `{{PRIME_NAME}}` to a real, selected candidate, fill the remaining
>   placeholders, delete every RENDER NOTE, do a final honesty pass, and only then is it a sendable document.
>
> This mirrors the gating already in `../13503-quantum-pnt/consortium-target-map.md` (search parked) and the
> founder-gated rows in each tender's `TENDER-…-PURSUIT.md`.

## Contents

| File | What |
|------|------|
| [`TEMPLATE.md`](TEMPLATE.md) | The parameterised 1–2pp brief. Fill-in `{{PLACEHOLDER}}`s; a full placeholder schema is in the comment block at the foot of the file. |
| [`brief-archetype-LSI-prime.md`](brief-archetype-LSI-prime.md) | Pre-rendered for the **LSI / system-integrator prime** class (owns SE, ECSS, submission, consortium mgmt). |
| [`brief-archetype-quantum-hardware-anchor.md`](brief-archetype-quantum-hardware-anchor.md) | Pre-rendered for the **quantum-hardware anchor** class (atomic-clock / cold-atom developer; mandatory quantum-nav Key Person source) — 13503-leaning. |
| [`brief-archetype-AI-payload-partner.md`](brief-archetype-AI-payload-partner.md) | Pre-rendered for the **AI / digital-payload partner** class (on-board AI / edge inference, SDR/FPGA payload, RF/antenna) — 13494-leaning. |

## What each brief renders (the five required sections)

Every brief — template and archetypes — carries the same five load-bearing elements from A13:

1. **The WP we own** (and only that) — fixed-scope SME seat, explicit in/out scope.
2. **The score-delta sheet** — the 20/30/30/10/10 criteria each marked ↑, plus the consortium-composition arithmetic (DE-SME → ≥3-SME quota + ≥30%-non-LSI + DE geo-return) and the registered-office item disclosed honestly.
3. **The audit-the-core hook** — the open Apache-2.0 core a prime can read line-by-line + the verification matrix that refuses "validated" without an external oracle, versus the closed-simulator incumbents.
4. **The validated-or-modelled capability table** — every row tagged **validated** (external oracle) or **modelled**, with the four partner-owned hardware/PA gaps carrying no claim.
5. **A clean BIPR / IP line** — Apache-2.0 background, no copyleft in the tree, ESA prior-authorisation route, Foreground/Pro split.

## How to use (on founder go)

1. Copy `TEMPLATE.md` to a new file named `brief-<prime-slug>-<tender-ref>.md`, **or** start from the closest archetype brief.
2. Bind `{{PRIME_NAME}}` and `{{TENDER_*}}` to the **selected** candidate and tender.
3. Fill the remaining placeholders (schema at the foot of `TEMPLATE.md`). Pull capability rows straight from the open repo `docs/CAPABILITY.md` and `src/verification.rs` — do **not** invent or upgrade a status.
4. Delete every `RENDER NOTE` and `ARCHETYPE NOTE`.
5. Honesty pass (see below). Only after that is it a sendable leave-behind — and sending is still the founder's call.

A render script is intentionally **not** provided: substituting a dozen placeholders by hand is faster than maintaining tooling, and a manual pass forces the honesty review every time. If a script is ever wanted, it would be a trivial `{{KEY}}` → value substitution over `TEMPLATE.md`.

## Honesty guardrails (apply to every rendered brief)

- **TRL ~3**, software-model substrate. **No flight heritage, no agency contracts, no certification, no ESA endorsement.** Never imply otherwise.
- Every capability row tagged **validated** (independent external oracle) or **modelled** — never silently promoted. The five externally-validated islands are fixed: **SGP4/SDP4** (AIAA 2006-6753, 4.12 mm) · **reference frames & timescales** (SOFA/ERFA + Vallado, 0.1–4 mm) · **Allan/ADEV** (NIST SP 1065 / Stable32, 1e-4) · **integrity RAIM/ARAIM** (DO-229E/DO-316 + real IGS SP3 / Celestrak TLE) · **ranging-code Gold trade** (GPS C/A −23.9 dB). Everything else is modelled.
- The AI-evaluation testbed's *harness logic* is validated against closed-form AUC bounds, but its corpus is **synthetic** (parameter-grounded, not raw IQ) and its detectors are **published-method baselines, not SOTA** — a high AUC demonstrates the pipeline, not field performance.
- The quantum-trade engine **quantifies, never validates** a partner's device; the four partner-owned gaps (bus · RF-payload/antenna HW · quantum-payload HW · flight PA) carry **no claim, by design.**
- Ashforde is a **consortium-SME WP contributor — never prime, never a mandated Key Person.** Never claim to satisfy the consortium or Key-Personnel mandate alone.
- The **Estonia registered-office** eligibility item is disclosed up front (German-branch fix + ESA Q&A), not hidden.
- Business contact **contact@ashforde.org** only — never any personal address. Keep the ESA-STAR entity id out of any file at risk of leaving the private repo.

---
*Asset only. Prime selection and all outreach are **PARKED** until the founder explicitly says go. Contact: contact@ashforde.org.*
