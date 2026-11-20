<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Draft outline: NATO DIANA challenge proposal

**Status:** draft outline, not a submission. Programme facts and sources: [`PROGRAMMES.md`](PROGRAMMES.md) section 3. Next call date, IP terms and full eligibility are **UNKNOWN** and gate any real submission.
**Target challenge area:** Operational Resilience in Contested Environments [S9].
**Kshana facts used:** public repository README, ROADMAP and `docs/RECEIVER-TRUST.md` only. No partner names or data; no anti-jam or anti-spoof performance figures appear here, and none should be added.

## 1. Problem

Systems that depend on GNSS for position and timing are exposed to jamming, spoofing and loss of the signal. Operators need to know, before a deployment and with a result they can reproduce, how long a system stays within its timing and position budget when GNSS degrades, and need a way to judge whether a receiver's output can be trusted during an event. Today these studies are ad hoc, hard to repeat and hard to audit. `[FOUNDER: add a sourced statement of the defence user need; cite a public NATO or national document, do not invent one.]`

## 2. Solution

### Already done (public, on `main`)

- Kshana is an open-source (AGPL-3.0) Rust engine that replays GNSS failures (jamming, spoofing, clock holdover) and reports how long a system stays within budget. Runs are reproducible bit for bit from the scenario, seed and engine version. Capabilities are labelled VALIDATED, MODELLED or PARTNER (README).
- Shipped resilience tools: jamming link-budget model, time-spoof detector, multi-layer spoof detector (RAIM consistency, AGC power, signal-quality), RAIM with protection levels, GNSS/INS fusion, holdover Kalman filter (ROADMAP).
- A receiver-log assessment that scores a log with pre-stated events and tolerances, reports per-epoch states (nominal, degraded, untrusted) and each event's outcome (detected, late, missed, not-evaluable), with the log hash and every threshold in the result (`docs/RECEIVER-TRUST.md`). Its monitors are labelled MODELLED there.
- Interfaces: command line, Rust library, Python, WebAssembly, container image.

### What the grant would fund (`[FOUNDER: confirm each item before use]`)

- Scenario packs for contested-environment PNT studies, defined with test-centre input.
- Extending the receiver-log assessment to more receiver log formats and more monitors, with independently reviewed methodology (see `AUDIT-SCOPE.md`).
- Reproducible evidence packaging: signed result bundles that a test centre can re-run.
- Documentation and a worked example for allied test centres.

Out of scope, stated up front: no RF generation, no transmission, no jamming or spoofing waveform synthesis (existing engine policy).

## 3. Work packages (placeholders for duration and effort)

| WP | Title | Content | Duration | Effort |
|---|---|---|---|---|
| 1 | Requirements with test centre | Define scenarios and acceptance criteria with a DIANA test centre | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 2 | Scenario and monitor extension | New contested-environment scenario packs; additional log formats and monitors | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 3 | Independent methodology review | Commission review of the trust-score method; address findings | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 4 | Test-centre trial | Re-run reference scenarios and score test-centre recordings; compare predictions with observations | `[PLACEHOLDER]` | `[PLACEHOLDER]` |
| 5 | Release and dissemination | Open release under AGPL-3.0, documentation, report | `[PLACEHOLDER]` | `[PLACEHOLDER]` |

## 4. Deliverables

- D1 Requirements and acceptance-criteria note.
- D2 Released scenario packs and monitor extensions, with tests.
- D3 Independent review report and Ashforde OÜ response.
- D4 Test-centre trial report: stated agreements and disagreements between predictions and observations, with their VALIDATED or MODELLED labels.
- D5 Public documentation and worked examples.

## 5. Team

Ashforde OÜ, Tallinn, a private limited company. Roles only, no names: `[FOUNDER: engineering lead, validation lead, programme lead; state who is employed and who is subcontracted.]` Test-centre partner: `[TO BE AGREED; none named here]`.

## 6. Budget shape

| Line | Amount |
|---|---|
| Personnel | `[PLACEHOLDER]` |
| Subcontract: independent review | `[PLACEHOLDER]` |
| Test-centre access and travel | `[PLACEHOLDER]` |
| Other direct costs | `[PLACEHOLDER]` |
| Funding requested | `[PLACEHOLDER; DIANA states EUR 100,000 contractual funding for the accelerator, per S9]` |
| Co-funding | `[UNKNOWN whether required]` |

## 7. Risks

| Risk | Mitigation |
|---|---|
| DIANA IP terms conflict with open release under AGPL | Read the call terms before applying; counsel review; **do not apply if open release is restricted** |
| Export-control status of the software is undetermined | Complete classification before submission; `[FOUNDER: state current status]` |
| Kshana is a simulator, not a certified receiver or flight product; defence users may expect more | State limits plainly in the proposal; keep VALIDATED/MODELLED labels |
| Test-centre availability and recording access | Agree access in WP1; choose scenarios not dependent on one centre |
| Estimated effort exceeds a small team's capacity | Fixed scope per WP; cut WP2 scope first |
| Next call date unknown | Prepare the outline now; finalise when the date is published |
