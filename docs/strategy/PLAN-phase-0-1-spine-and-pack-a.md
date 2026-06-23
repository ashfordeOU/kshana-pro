# Phase 0 + Phase 1 Implementation Plan — Spine Completion & Pack A

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> (recommended) or `superpowers:executing-plans`, task-by-task. Steps use checkbox (`- [ ]`)
> syntax. **Every code task: Read the current file first** — line numbers below are indicative,
> the codebase moves. Follow `superpowers:test-driven-development` for every code task.

**Goal:** Finish the shared product spine (open + commercial) and ship Pack A "Optimism-Gap
Audit" as a buyable study, without re-implementing any existing engine.

**Architecture:** Open `kshana` gains a thin, reproducible *study* layer (labels, suite
aggregation, metadata, validation). Commercial `kshana-pro` wraps it with a branded report,
PDF, multi-pack orchestration, and a study-request motion; Pack A becomes a packaged offer.

**Tech stack:** Rust (both repos; `kshana-pro` depends on `kshana` as a Cargo dep, never forks).
Web: existing `kshana/web` (vanilla JS/MJS). Legal/GTM: Markdown. PDF: headless-Chromium or
`typst` (decided in Task P4).

**Constraints:** AGPL-3.0-only headers on open files (`// SPDX-License-Identifier: AGPL-3.0-only`).
No AI/automation attribution in any commit or file. Commits authored `Chakshu Baweja
<contact@ashforde.org>`, no Co-Authored-By footer. Never the personal email. Push only when the
founder says so. Keep `MODELLED` / "not a certification" everywhere. Honesty gate
(`kshana/tests/no_overclaims.rs`) must stay green.

---

## PHASE 0 — Spine completion

### Task O1: Surface per-FoM MODELLED/VALIDATED label in the open report

**Why:** Labels exist in the verification matrix but the report shows naked FoM numbers. Map
each FoM name → its validation tier and render it inline. No change to `FoMScores` itself.

**Files:**
- Read: `kshana/src/verification.rs` (the matrix), `kshana/src/fom.rs` (`FoMScores` fields),
  `kshana/src/api.rs` (`html_report`), `kshana/web/report.mjs`
- Create: `kshana/src/fom_label.rs` (FoM-name → `ValidationTier` lookup)
- Modify: `kshana/src/api.rs` (render label column), `kshana/src/lib.rs` (module decl),
  `kshana/web/report.mjs` (label column in browser path)
- Test: `kshana/tests/fom_label_reference.rs`

- [ ] **Step 1 — failing test:** assert every `FoMScores` field maps to a tier, and that
  `holdover_s` resolves `Modelled` while a known real-data-validated metric resolves `Validated`
  (pick the exact pair from `verification.rs`).
```rust
// kshana/tests/fom_label_reference.rs
use kshana::fom_label::{tier_for, ValidationTier};
#[test]
fn every_fom_has_a_tier_and_holdover_is_modelled() {
    for name in ["timing_rms_ns","timing_p95_ns","holdover_s","resilience_slope_ns_per_s",
                 "availability","integrity","security"] {
        assert!(tier_for(name).is_some(), "no tier for {name}");
    }
    assert_eq!(tier_for("holdover_s"), Some(ValidationTier::Modelled));
}
```
- [ ] **Step 2:** run `cargo test -p kshana --test fom_label_reference` → FAIL (module missing).
- [ ] **Step 3:** implement `fom_label.rs`: `enum ValidationTier { Validated, Modelled, Partial }`
  + `pub fn tier_for(fom: &str) -> Option<ValidationTier>` sourcing tiers from `verification.rs`
  (single source of truth — do not hardcode a second matrix).
- [ ] **Step 4:** run test → PASS.
- [ ] **Step 5:** render the tier next to each value in `api.rs` `html_report` FoM table and in
  `web/report.mjs`; add `report.test.mjs` assertion that the label renders.
- [ ] **Step 6:** `cargo test -p kshana && (cd kshana/web && npm test)` → PASS; honesty gate green.
- [ ] **Step 7:** commit `feat(report): surface per-FoM validation tier in study reports`.

### Task O2: Additive study-metadata block (does NOT perturb scenario hash)

**Why:** Reports need study title / generation time / optional author + disclaimer without
breaking reproducibility (`hash_scenario` hashes scenario *inputs* only).

**Files:**
- Read: `kshana/src/report.rs` (`RunResult`, `hash_scenario`)
- Modify: `kshana/src/report.rs` (add `StudyMeta` + optional `meta` field, `#[serde(skip_serializing_if="Option::is_none")]`)
- Test: `kshana/tests/study_meta_hash_stability.rs`

- [ ] **Step 1 — failing test:** building a `RunResult` with vs without `meta` yields the **same**
  `scenario_hash` (meta is output-only, like `eci_track`).
- [ ] **Step 2:** run → FAIL (`StudyMeta`/`meta` missing).
- [ ] **Step 3:** add `pub struct StudyMeta { study_title: Option<String>, generated_utc: Option<String>,
  author: Option<String>, disclaimer: Option<String> }` and `pub meta: Option<StudyMeta>` on
  `RunResult`, serde-skipped when `None`; confirm `hash_scenario` only hashes the `Scenario`.
- [ ] **Step 4:** run → PASS.
- [ ] **Step 5:** commit `feat(report): optional output-only study metadata (hash-stable)`.

### Task O3: Generation timestamp + study title in provenance footer & HTML title

**Files:** Read/Modify `kshana/src/api.rs` (footer `with_provenance`, `html_report` title);
Test `kshana/tests/provenance_footer.rs`.

- [ ] **Step 1 — failing test:** when `StudyMeta.generated_utc` + `study_title` are set, the HTML
  `<title>` contains the study title and the footer contains the timestamp; when unset, falls back
  to current generic strings (back-compat).
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3:** thread `meta` into footer + title; timestamp is *passed in* (caller supplies ISO
  string — keep `api` pure/deterministic; do not call a clock inside the engine).
- [ ] **Step 4:** run → PASS. **Step 5:** commit `feat(report): study title + generation stamp`.

### Task O4: `--study-name` CLI flag → HTML title + output filename

**Files:** Read/Modify `kshana/src/main.rs` (arg parser shown at top of file: positional + flags),
optionally `kshana/src/run.rs`; Test `kshana/tests/cli_study_name.rs` (or extend `tutorials.rs`).

- [ ] **Step 1 — failing test:** invoking the run path with a study name sets `StudyMeta.study_title`
  and the output base name (assert at the API/builder level the name propagates).
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3:** add `--study-name <s>` to the `while i < args.len()` match in `main.rs`; pass into
  the `StudyMeta`; use it for the `.report.html` / `.result.json` base name when present.
- [ ] **Step 4:** run → PASS. **Step 5:** commit `feat(cli): --study-name for named study artifacts`.

### Task O5: Scenario *suite* manifest format + parser

**Why:** Group scenarios into one coherent study (Pack-A audit = N scenarios). Define a small
`suite.toml`: `title`, `description`, `scenarios = ["a.toml","b.toml"]`, optional per-entry label.

**Files:** Create `kshana/src/suite.rs`; Modify `kshana/src/lib.rs`; Test `kshana/tests/suite_parse.rs`;
Fixture `kshana/tests/fixtures/suite-min.toml`.

- [ ] **Step 1 — failing test:** parse a 2-scenario `suite.toml` → `Suite { title, scenarios: [..] }`;
  a missing `scenarios` key is an error with a clear message.
- [ ] **Step 2:** run → FAIL. **Step 3:** implement `Suite` + `parse_suite(src) -> Result<Suite,..>`.
- [ ] **Step 4:** run → PASS. **Step 5:** commit `feat(suite): scenario-suite manifest format`.

### Task O6: `study` mode — run a suite → aggregated `study.json` + comparison HTML

**Files:** Modify `kshana/src/api.rs` (add `run_suite(suite, base_dir) -> StudyOutput`), reuse
existing per-scenario `run_toml`; Create `kshana/src/study.rs` (aggregation + side-by-side render);
Modify `kshana/src/main.rs` (`kshana --study suite.toml`); Test `kshana/tests/study_aggregate.rs`.

- [ ] **Step 1 — failing test:** `run_suite` over a 2-scenario suite returns a `StudyOutput` whose
  per-scenario results match running each alone (determinism), and whose HTML contains both titles
  and a comparison table.
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3:** implement: iterate scenarios → collect `RunResult`s → aggregate into `StudyOutput
  { meta, results, comparison_svg }`; render a side-by-side FoM table (reuse O1 labels). No new engine.
- [ ] **Step 4:** run → PASS.
- [ ] **Step 5:** wire `--study` in `main.rs`; write `study.json` + `study.html`. Run `cargo test -p kshana`.
- [ ] **Step 6:** commit `feat(study): one-command multi-scenario study aggregation`.

### Task O7: `--validate` — scenario linting before a long run

**Files:** Modify `kshana/src/api.rs` (`validate_scenario(src) -> Vec<String>`, reuse
`ScenarioKind::classify` + per-kind required fields from `ScenarioMeta`); Modify `kshana/src/main.rs`
(`kshana --validate scenario.toml` → exit 0 clean / 1 with violations); Test `kshana/tests/validate_scenario.rs`.

- [ ] **Step 1 — failing test:** a scenario missing a required field returns a non-empty violation
  list naming the field; a good scenario returns empty.
- [ ] **Step 2:** run → FAIL. **Step 3:** implement using existing `ScenarioMeta` required-field data.
- [ ] **Step 4:** run → PASS. **Step 5:** wire CLI flag. **Step 6:** commit `feat(cli): --validate scenario linting`.

### Task O8: Honesty gate stays green (verification, not a feature)

- [ ] Run `cargo test -p kshana --test no_overclaims`. If any new public surface (e.g. new docs)
  was added, ensure it carries no retired-overclaim phrase. No new claims about TRL/certification.
- [ ] Full open gate: `cargo fmt --check && cargo clippy -- -D warnings && cargo test -p kshana`.

---

### Task P1: Branded cover + executive summary in the commercial report

**Why:** The commercial report is generic. Add an Ashforde-branded cover (wordmark/seal),
a one-page executive summary, and a MODELLED banner — the difference a customer pays for.

**Files:**
- Read: `kshana-pro/src/report.rs` (`ConsolidatedReport`, Markdown/HTML renderers)
- Modify: `kshana-pro/src/report.rs` (add `cover` + `exec_summary` sections to the HTML/MD render)
- Assets: reuse Ashforde brand tokens (navy/brass/ivory, Cormorant/EB Garamond, seal) — embed
  inline (no external fetch) so the report stays self-contained
- Test: `kshana-pro/tests/report_cover.rs`

- [ ] **Step 1 — failing test:** rendered HTML contains the study title, "Ashforde", a `MODELLED`
  banner string, and "not a certification"; rendered with no cover data still produces a valid report.
- [ ] **Step 2:** run `cargo test -p kshana-pro --test report_cover` → FAIL.
- [ ] **Step 3:** implement cover + exec-summary blocks; MODELLED banner + disclaimer are
  non-optional on the commercial face.
- [ ] **Step 4:** run → PASS. **Step 5:** add a honesty assertion to an existing `kshana-pro`
  no-overclaim test (disclaimer present). **Step 6:** commit `feat(report): branded cover + exec summary`.

### Task P2: Inline per-FoM label in the commercial report (reuse O1)

**Files:** Modify `kshana-pro/src/report.rs` to call `kshana::fom_label::tier_for`; Test extend
`report_cover.rs`.
- [ ] Failing test: a holdover row renders "(MODELLED)" inline → implement → PASS → commit
  `feat(report): inline validation tier on every figure of merit`.

### Task P3: Provenance / study-metadata block (study id, customer, date, engine+scenario hash, classification)

**Files:** Modify `kshana-pro/src/report.rs` (`StudyProvenance { study_id, customer, prepared_for,
prepared_utc, engine_version, scenario_hash, classification }`); Test extend `report_cover.rs`.
- [ ] Failing test: block renders all fields + the scenario hash from the underlying `RunResult` →
  implement → PASS → commit `feat(report): audit-grade provenance block`.

### Task P4: PDF export of the report

**Decision step first** (record in the task): prefer **`typst`** (deterministic, no browser, fast)
unless an HTML→PDF path (headless Chromium / `wkhtmltopdf`) is required for fidelity to the HTML.
Pick one, note why, keep it a thin module.

**Files:** Create `kshana-pro/src/report/pdf.rs` (or `report_pdf.rs`); Modify `kshana-pro/src/main.rs`
(a `report --pdf <out.pdf>` path); Test `kshana-pro/tests/report_pdf_smoke.rs`.
- [ ] **Step 1 — failing test:** rendering a report to PDF writes a non-empty file beginning with
  `%PDF`. **Step 2:** FAIL. **Step 3:** implement chosen path. **Step 4:** PASS. **Step 5:** commit
  `feat(report): PDF export of branded study report`.

### Task P5: `ProStudy` multi-pack orchestration

**Why:** Compose Pack A + B + C results into one coherent study (the "full PNT-resilience audit").

**Files:** Create `kshana-pro/src/study.rs` (`ProStudy { pack_a: Option<..>, pack_b: Option<..>,
pack_c: Option<..> }` + a renderer that emits per-pack sections through `report.rs`); Modify
`kshana-pro/src/lib.rs`, `kshana-pro/src/registry.rs` (dispatch a `pro-study` kind); Test
`kshana-pro/tests/pro_study.rs`.
- [ ] **Step 1 — failing test:** a `ProStudy` with only Pack A set renders a Pack-A section and
  omits B/C cleanly; with all three set, renders three labelled sections + one cover.
- [ ] **Step 2:** FAIL. **Step 3:** implement (compose existing pack outputs; do not re-run engines
  twice). **Step 4:** PASS. **Step 5:** commit `feat(study): multi-pack ProStudy orchestration`.

### Task P6: Study-request intake schema + `POST /study`

**Files:** Create `kshana-pro/src/serve/study_intake.rs` (`StudyRequest { pack, customer,
prepared_for, inputs }` + validation); Modify `kshana-pro/src/serve/mod.rs` (route `POST /study`,
entitlement-gated like `/run`); Test `kshana-pro/tests/study_intake.rs`.
- [ ] **Step 1 — failing test:** a valid Pack-A `StudyRequest` parses + validates; an unknown pack
  or missing customer is rejected with a clear error; the route is gated by tier.
- [ ] **Step 2:** FAIL. **Step 3:** implement intake + route (reuse existing license gating in
  `license/mod.rs`). **Step 4:** PASS. **Step 5:** commit `feat(serve): study-request intake endpoint`.

### Task P7: Study tracking ledger (request → run → deliver, hash-stamped)

**Files:** Create `kshana-pro/src/study_ledger.rs` (append-only records: `study_id`, request hash,
run hash, delivered_utc; integrity hash chaining like the existing provenance pattern); Test
`kshana-pro/tests/study_ledger.rs`.
- [ ] Failing test: appending a record yields a stable record hash; tampering with an earlier
  record breaks chain verification → implement → PASS → commit `feat(study): tamper-evident study ledger`.

- [ ] **Phase 0 gate:** `cargo fmt --check && cargo clippy -- -D warnings && cargo test` in **both**
  repos green; open honesty gate green. (No push — local only, await founder.)

---

## PHASE 1 — Pack A "Optimism-Gap Audit" → shippable

### Task A1: `pro-optimism-gap-audit` pack

**Files:** Create `kshana-pro/src/packs/optimism_gap_audit.rs` (an `ExternalPack` wrapping
`kshana::impairment_study`); Modify `kshana-pro/src/packs/mod.rs`, `kshana-pro/src/registry.rs`
(register kind `pro-optimism-gap-audit`); Test `kshana-pro/tests/optimism_gap_audit_pack.rs`.
- [ ] **Step 1 — failing test:** running the pack on the published baseline corpus reproduces the
  optimism-gap headline numbers within tolerance (R²/gap) from the existing study.
- [ ] **Step 2:** FAIL. **Step 3:** implement the pack (call the open engine; do not duplicate it).
- [ ] **Step 4:** PASS. **Step 5:** commit `feat(packs): pro-optimism-gap-audit pack`.

### Task A2: Customer-detector intake format + adapter (bring-your-own detector)

**Why:** Today the audit runs the published fused baseline. Pack A's value is auditing *the
customer's* detector. Define a simple intake: the customer supplies per-sample detector scores
(or a scored corpus) in a documented schema; we compute their ID-vs-OOD gap.

**Files:** Create `kshana-pro/src/packs/detector_intake.rs` (schema + parser + adapter into the
open `impairment_study` gap computation); Doc `kshana-pro/docs/packs/DETECTOR-INTAKE-FORMAT.md`;
Test `kshana-pro/tests/detector_intake.rs`.
- [ ] **Step 1 — failing test:** a minimal customer-scores file parses and produces an ID-vs-OOD
  gap via the open engine; malformed input is rejected with a clear message.
- [ ] **Step 2:** FAIL. **Step 3:** implement schema + adapter. **Step 4:** PASS. **Step 5:** commit
  `feat(packs): customer-detector intake for optimism-gap audit`.

### Task A3: Pack-A report template (field-vs-bench framing)

**Files:** Modify `kshana-pro/src/report.rs` or add `kshana-pro/src/report/pack_a.rs` (a Pack-A
section: headline "predicted field-vs-bench degradation", optimism-gap figure, R²/p, per-class
table, MODELLED labels, "not a certification"); Test `kshana-pro/tests/report_pack_a.rs`.
- [ ] **Step 1 — failing test:** the Pack-A section renders the gap headline + R²/p + a per-class
  table + the MODELLED/disclaimer banner. **Step 2:** FAIL. **Step 3:** implement. **Step 4:** PASS.
- [ ] **Step 5:** commit `feat(report): Pack-A optimism-gap study template`.

### Task A4: Pack-A scenario templates

**Files:** Create `kshana-pro/scenarios/optimism-gap-baseline.toml` (+ an ID/OOD sweep template);
Test `kshana-pro/tests/pack_a_templates.rs` (templates parse + run through the pack).
- [ ] Failing test → implement templates → PASS → commit `feat(scenarios): Pack-A study templates`.

### Task A5 (doc): Pack A one-pager

**File:** Create `kshana-pro/docs/packs/PACK-A.md`. Not TDD — content deliverable.
- [ ] ICP (GNSS spoof/jam/RF-impairment detector R&D teams); JTBD ("know field-vs-bench degradation
  before you ship"); what they send (detector intake) + what they get (the sample report); the
  arXiv:2606.22054 credibility hook (preprint, not peer-reviewed); MODELLED + "not a certification";
  CTA `contact@ashforde.org`. **Acceptance:** a non-expert can read it and know whether Pack A is for
  them and how to start.

### Task A6 (doc): Sample study report (Pack A, from published data)

**File:** Generate `kshana-pro/docs/packs/sample-optimism-gap-study.{html,pdf}` from the published
optimism-gap artifact via Tasks P1–P4 + A3. Not TDD — generated deliverable.
- [ ] **Acceptance:** the sample is fully branded, MODELLED-labelled, provenance-stamped, reproducible
  from the public artifact, and contains no customer data and no overclaim. This is the thing
  prospects see before buying.

### Legal Tier-1 (parallel doc track — drafts "subject to counsel")

**Files (create in `kshana-pro/legal/`):**
- [ ] `COMMERCIAL-LICENSE.md` — EULA: grant, restrictions, warranties, liability cap, IP indemnity,
  termination; per-tier (Eval/Pro/Enterprise) consistent with `src/license/`.
- [ ] `PRICING.md` — rate card structure for Pack A study + day-rate + add-ons (figures **founder-set**;
  leave clearly-marked placeholders, do not invent numbers).
- [ ] `DATA-PROCESSING-AGREEMENT.md` — data location, retention, deletion-on-termination, audit rights,
  incident response (customer detector/threat inputs).
- [ ] `SOW-TEMPLATE.md` — study objective, scope, deliverables (report + evidence JSON + scenario hash),
  timeline, acceptance criteria, IP ownership, change control.
- [ ] **Acceptance:** each carries a "DRAFT — subject to counsel review" header; `contact@ashforde.org`;
  no personal email; consistent with existing `legal/SUPPORT-SLA.md` + `IP-POLICY.md`.

### Web: "Request a study" CTA + intake form on kshana.dev

**Files:** Modify `kshana/web/index.html` (Editions/Commercial section — add a "Request a study"
CTA), `kshana/web/style.css`; optionally `kshana/web/study-request.html` (a simple mailto/form to
`contact@ashforde.org` — no third-party tracker, privacy-preserving); Test add `kshana/web/*.test.mjs`
coverage if a script is added.
- [ ] **Step 1 — failing test (if JS added):** the CTA renders + points to the intake. **Step 2:** FAIL.
- [ ] **Step 3:** implement CTA (honest copy: "MODELLED study", "not a certification"). **Step 4:** PASS.
- [ ] **Step 5:** run honesty gate (`no_overclaims.rs` scans `web/index.html`) → green. **Step 6:**
  commit `feat(web): request-a-study CTA`.

- [ ] **Phase 1 gate:** both repos green (fmt/clippy/test); honesty gate green; sample report renders;
  Pack-A one-pager + Tier-1 legal drafts present. **No push — await founder go.**

---

## Execution

Per `superpowers:writing-plans` handoff, choose:
1. **Subagent-driven** (recommended) — fresh subagent per task + two-stage review.
2. **Inline** — batch execution with checkpoints.

Order: O1→O8, then P1→P7 (Phase-0 gate), then A1→A6 + Legal Tier-1 + Web (Phase-1 gate). Phases 2
(Pack C) and 3 (Pack B) get their own detailed plans once Phase 1 has a paying reference and (for B)
export-control clearance. **Nothing is pushed without an explicit founder go.**
