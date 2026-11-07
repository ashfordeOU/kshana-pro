# Changelog

All notable changes to the private Kshana Pro overlay. Each lane keeps its entry in its
own bullet so parallel work merges cleanly.

## [Unreleased]

- **Kshana Desktop Pro (F10e, L2a Pro build).** `apps/desktop-pro`: the free Desktop's shell and
  command names with the Pro overlay; Pro kinds run under the `desktop` licence; a licence page
  (status, offline activation, machine ID, removal) and an Upgrade offer only while payments are
  on. Its own workflow (`desktop-pro.yml`). Unsigned.

- **Free-build check (F10e).** `scripts/free-build-check.sh` fails on any binary carrying a Pro
  marker (the crate name, a Pro kind or feature name, the proprietary notice); a test runs it on the
  Pro binary, where it must fail. The free Kshana Desktop built from the open repository's patch
  passes it (no marker). The systemd settings file is `serve.conf` (`*.env` stays ignored).

- **Linux package and air-gapped bundle (L6, L7).** `scripts/build-deb.sh` (binary, hardened systemd
  unit, settings, guides; unsigned) and `scripts/offline-bundle.sh` (binary, unit, guides, revocation
  list, CycloneDX SBOM from the locked graph via `scripts/sbom.py`, `SHA256SUMS`, an optional Ed25519
  signature over every file, and an `install.sh` that checks everything before installing and
  activates a licence offline). `tests/offline_bundle.rs`. No `.rpm` yet; Windows MSI not built here.

- **Station fleet (L3 Pro part, F10e).** `POST /fleet/stations`, `GET /fleet/stations`,
  `POST /fleet/stations/<id>/heartbeat|retire` on the Pro service: Stations register and report
  health (live, stale, registered, retired). One Station without `pro-station-fleet`; with it, the
  licence's `stations` claim caps the active Stations; retiring frees a place. The Station-fleet
  edition now covers the `service` product. `tests/station_fleet.rs`.

- **Saved studies and share links (F3a).** `POST/GET /library`, `POST /library/<name>/run`: named
  job requests per tenant. `POST /jobs/<id>/share` makes a read-only link (`/shared/<token>`, 1 to
  90 days, token shown once, only its hash stored) that anyone can open without a credential;
  `POST /shares/<token>/revoke`. Clients regenerated.

- **Hosted trial sandbox (F3b).** `serve --trial`: open to anyone, Evaluation tier whatever the
  licence, one tenant per caller (salted hash of the address, never stored), a daily job quota
  counted from the job store, a capped time limit, a per-minute request limit, a server-wide queue
  cap; `/run`, `/study` and admin routes closed. Public hosting needs the owner's machine and
  domain. `tests/service_trial.rs`.

- **Spectrum coexistence with other systems (F8b).** `[[other_system]]` entries (signal design and
  constellation, like plans) add their upper-bound interference to each plan's per cell and victim;
  the result gives the worst cell of the sum, its verdict and the plan's share. Tested against the
  closed form (an identical incumbent doubles the ratio, share 0.5). GPS L2C as a victim and a
  receive-antenna pattern remain gaps: the first waits on an open-engine `leo-signal` model, the
  second on per-satellite elevations the coverage runs do not give.

- **Unicode in the mission dossier PDF (F8c).** A dossier with text the PDF standard fonts cannot
  show is set in Noto Sans (OFL 1.1, `assets/fonts/`), embedded with a Unicode map so the text is
  searchable; the footer's honesty line stays a built-in-font literal. ASCII dossiers are
  unchanged (the face is embedded only when used). Latin, Greek and Cyrillic; other scripts are a
  stated gap.

- **Correlated inputs in the uncertainty study (F8a).** `[[correlation]] inputs = [a, b], rho, source`
  correlates inputs through a Gaussian copula (Cholesky factor of the normal-score correlation
  matrix); marginals stay exact; inconsistent matrices are refused; `src` and `sobol` sensitivity
  are refused with correlations (both assume independence). The result lists the correlations and
  the summary flags assumed ones; a study without correlations keeps its hash.

- **Study funnel, Pack C subscription, STK note (F3c, F5, F8d).** Every `POST /study` with a data
  directory is recorded in `studies.jsonl`; `/metrics` adds `kshana_study_requests_total` by outcome
  and `kshana_study_dossiers` by job state. Pack C: the `pack-c-ci-monthly` edition (price
  `PLACEHOLDER`), the subscriber guide `docs/packs/PACK-C-CI.md` with GitHub Actions and GitLab CI
  jobs (`examples/ci/`), and `tests/pack_c_subscription.rs` (purchase webhook, gated CI runs,
  renewal, refund). The connectors note no longer says STK `.e` is missing from the open core
  (it has `kshana::interop::stk` since 0.29); a test guards that.

- **Service hardening and SLA metrics (F2a, F2b).** `serve --tls-cert/--tls-key` serves HTTPS
  from the binary (rustls, ring provider). `POST /jobs` takes `priority` (high, normal, low) and
  `timeout_secs`; `POST /jobs/<id>/cancel` cancels a queued or running job (new state
  `cancelled`); `GET /jobs/<id>/events` streams status changes as server-sent events. High
  availability: `--ha-node` nodes share one data directory, one holds a lease and runs jobs, a
  standby takes over when it expires (Enterprise, `pro-service-ha`); each node has its own audit
  log. `kshana-pro backup create|verify` copies a data directory while it serves and checks the
  copy. `GET /metrics`: Prometheus text per tenant (jobs by state, queue wait and run time
  quantiles, requests by route and status). Clients regenerated; the TypeScript client test imports
  an `.mts` copy so Node reads it as an ES module on every machine. `tests/service_hardening.rs`.

- **Licensing for 0.41 (F10b, F10c, F10d, F10f).** Licence token v2: products (cli, desktop,
  station, service), seats, Stations, optional machine binding (salted hash of the hardware ID,
  `license machine-id`), key ID with several trusted keys for rotation and retired keys refused,
  not-before; v1 tokens verify unchanged. Offline activation (`license activate|status|
  deactivate`) into a per-user store, warnings at 30 and 7 days, a 14-day grace period, signed
  revocation lists that arrive with updates and cannot be rolled back, and an opt-in online
  refresh that is off by default. Vendor issuing: a hash-chained issuance register with trials,
  renewals, revocations and reminders (`license trial|renew|revoke|revocations build|reminders|
  register verify`), and the key custody and rotation procedure (`docs/LICENSING-KEYS.md`). Every
  Pro kind and command now has an explicit tier, enforced by a test; the 0.41 edition matrix adds
  `pro-station-fleet`, `pro-service-ha`, `pro-desktop` and `pro-signed-dossier`. Without
  `--license`, commands use the activated token. `docs/LICENSING.md`, `tests/licensing.rs`.
- **Payments and paywall, off (F10g).** `kshana-pro payments check|checkout|serve`: Paddle and Lemon
  Squeezy webhooks verified by their published HMAC schemes issue, renew and revoke tokens through
  the register and rewrite the revocation list; forged, stale, replayed and out-of-order deliveries
  are refused. `commercial/payments.toml` keeps `enabled = false` and every price `PLACEHOLDER`;
  live mode is refused until the owner sets them (gate 12). No outbound network call in any state.
  `docs/PAYMENTS.md`, `tests/payments.rs`.
- **Signed dossier (F7).** `kshana-pro dossier keygen|sign|verify`: every file under a dossier (or
  any delivery directory) under one Ed25519 signature; verification is offline and needs no
  licence, and `scripts/verify-dossier-signature.py` checks it without Kshana.
  `tests/signed_dossier.rs`.

- **Open engine 0.40.0 (F1a).** The `kshana` pin moves from `=0.34.1` to the published `=0.40.0`
  (crates.io) with its lockfile, and `rust-version` to the engine's 1.88. One overlay source line:
  `benefits::oracle_str` names the engine's new `IntegrationRun` oracle kind. What moved, each
  traced to an engine change:
  - The broadband jammer Q went from 1 to its textbook 2 in engine 0.36 (effective C/N0 up to
    3.01 dB higher under broadband jamming, every jamming threshold 3.01 dB higher). In the
    `campaign-baseline` pack the jamming phase now ends at 30.6 dB-Hz (was 27.7, all 8
    satellites still tracking), and availability falls below one half at -20 dBW, so that
    watch's requirement band moves from [-30, -25] to [-25, -20] dBW. `docs/CAMPAIGN-WATCH.md`
    and the open engine's `docs/CAMPAIGNS.md` carry the new numbers.
  - The engine's `run_scenario` now appends a `figure_tiers` block that a registry-built pack
    does not, so `tests/open_engine_reachable.rs` compares the registry path with the engine's
    untiered run (the Pro dispatcher still matches the full document). Scenario files whose data
    stays in the engine repository and is not in the published crate (the LOLA crop and the
    DE440s Moon kernel) fall through to the next file of their kind; all 92 kinds are proven.
  - The paper reports were regenerated with `emit_paper_artifacts`: five HTML files change only
    in their engine-version footer (checked by substitution); the frame-EOP figure gains the
    engine's evidence footer (16 px taller); the cislunar observability table's numerically zero
    eigenvalue at the rank-2 epoch moves from -6.8e-24 to -1.0e-23, which moves its CSV digest
    in the report. No other published number moved.

- **Open engine 0.34.1.** The `kshana` pin moves from `=0.34.0` to the published `=0.34.1`
  (crates.io) with its lockfile; no overlay source needed changing. The version-stamped paper
  reports under `papers/` (and `papers/realtime-frame-eop/fig2_frame_eop.svg`) were
  regenerated with the `emit_paper_artifacts` generators; each HTML and SVG changes only in its
  engine-version footer (checked: the old file with `0.34.0` replaced by `0.34.1` is
  byte-identical to the new one), so no published number moved. The lab-campaign
  fixture was regenerated with the published 0.34.1 binary: all 12 cells match the 0.34.0 ones
  field for field except `engine_version`, which is part of the cell key, so the keys and the
  DIGEST move (`a76ec0cd…` to `01ab3ca9…`).
- **Open engine 0.34.0.** The `kshana` pin moves from `=0.33.1` to the published `=0.34.0`
  (crates.io) with its lockfile; no overlay source needed changing, and `cargo test --all
  --locked` passes. The six version-stamped paper reports under `papers/` (and the figure
  `papers/realtime-frame-eop/fig2_frame_eop.svg`) were regenerated with the
  `emit_paper_artifacts` generators: each HTML and SVG changes only in its engine-version
  footer (checked: the old file with `0.33.1` replaced by `0.34.0` is byte-identical to the
  new one), so no published number moved. Four report PDFs were regenerated with them.
- **Open engine 0.33.1.** The `kshana` pin moves from `=0.33.0` to the published `=0.33.1`
  (crates.io) with its lockfile; no overlay source needed changing. 0.33.1 is the engine's
  security release (chart text escaping and the Studio's handling of untrusted scenario text),
  with no change to any model or published number. The six version-stamped paper reports
  under `papers/` were regenerated with the `emit_paper_artifacts` generators and change only
  in their engine-version footer.
- **Open engine 0.33.0.** The `kshana` pin moves from `=0.32.0` to the published `=0.33.0`
  (crates.io) with its lockfile; no overlay source needed changing, and `cargo test --all
  --locked` passes. The six version-stamped paper reports under `papers/` were regenerated
  with the `emit_paper_artifacts` generators: each report changes only in its engine-version
  footer, and one figure (`papers/realtime-frame-eop/fig2_frame_eop.svg`) also takes the
  engine's new Observatory colours and font, with every plotted coordinate and label
  unchanged.
- **Lab campaign report, step L1 + data-hygiene guard (L6)** (plan
  `docs/plans/2026-10-07-lab-campaign-report-plan.md`). New `lab_campaign` module reads the
  engine's 0.34.0 lab-replay output (`campaign.json`, `cells/<key>.json`, `DIGEST`; public
  `docs/design/LAB-CAMPAIGN.md`) into strict serde mirror types. Before anything is rendered it
  re-derives every cell key, recomputes the DIGEST, and checks the schema versions, engine
  version and campaign membership, and that the `MODELLED` label is on the reference curve.
  An incomplete campaign is reported, not hidden. `campaign.json`'s shape and the DIGEST byte
  form are provisional until the engine side confirms them. Export-control item N6: a new guard
  test fails on any campaign or test-condition document outside the synthetic fixtures, and on
  any SigMF recording anywhere; `.gitignore` blocks campaign output. Fixture: a hand-made
  synthetic campaign (2 recordings × 2 chains × 3 designs = 12 cells), written by `make_fixture.py`.
  Step L2 (`lab_campaign::aggregate`) selects and orders the cells' numbers for the report.
  It computes no metric. Every derived figure is a declared median, sum, count or ratio of
  sums, and carries its input count. Scorecard metrics are whole-run per (chain, design);
  event metrics stay per event, so a jamming ramp and an outage are never pooled.
  J/S bins with no locked epochs stay visible next to their MODELLED value.
  Step L3 (`report_lab_campaign`) renders the branded HTML report: cover with data-class
  banner and verified DIGEST, template-only summary, test matrix, scorecard and heatmap,
  per-event results, measured C/N0-vs-J/S curves beside the dashed MODELLED reference,
  stability, pass/fail (only when the client set bars), definitions, provenance, limits and
  appendices. Every number in a data table carries `data-v`/`data-op`/`data-n`, and a test traces
  each one back to the cells. Step L5 adds `kshana-pro lab-report <dir> --data-class … --html …`
  (Pro entitlement, as `study`). It refuses a data-class mismatch and an incomplete campaign
  unless `--allow-incomplete` is given. A client name appears only from `--client`, and
  client-confidential renders carry the export-control notice placeholder (N6).
  Step L4 (`report_lab_campaign_pdf`) is the A4 PDF edition (`--pdf <out.pdf>`). It has the
  same sections and the same summary sentences, from one shared builder, with the curves as
  tables. Table headers stay with their rows, and a subheading never ends a page. Two renders
  are byte-identical apart from printpdf's random trailer `/ID`.
  **Aligned to the engine's real output** (public `kshana` 0.34.0, developed on `feat/034-campaign`, PR #46, last head `51ba4787`; the
  provisional markers are gone):
  - the mirror types are the engine's own structs: `kshana.campaign-plan/1`, cells with
    hand-off and lock source, and `kshana.campaign-scorecard/1`;
  - the cell key and the DIGEST use the engine's exact formulas;
  - pass/fail shows the engine's verdicts from `scorecard.json`, counted and never recomputed;
  - the method section states the cells' lock source and what it implies;
  - the fixture is regenerated as real engine output on synthetic recordings (`make_fixture.py`);
  - `data_class` is required in the plan, every cell (where it enters the key) and the
    scorecard, and must match `--data-class`;
  - the lock source is the track session's (`track-session`), and an unknown one is refused;
  - epoch files a cell names are checked for presence, size and SHA-256.

  Re-pin when the engine's default design changes and at 0.34.0.
- **Open engine 0.32.0** (PR 5, merge `586517f`; `68c64c1`). The `kshana` pin moves from
  `=0.29.0` to the published `=0.32.0` (crates.io) with its lockfile; the engine changes
  since 0.29.0 are additive and no overlay source needed changing. The six version-stamped
  papers under `papers/` (report HTML/PDF and figures) and the D1.4 MRD block were
  regenerated. The campaign-watch branch-point fixture was re-recorded: the 0.30.0 ARAIM
  revision moved `mean_hpl_m` and `mean_vpl_m`, and 0.32.0 writes the same values as
  0.30.0. Supersedes the unmerged `claude/pro-030` branch.
- **Campaign-watch fixture compares metrics within 8 ulp** (PR 5; `c3c20c1`, note
  corrected in `c99d0e7`). The check compared `ci.json` byte for byte, and the macOS
  arm64 runner rounds the ARAIM sums 1–2 ulp away from Linux x86-64 (`mean_tropo_m` 2 ulp,
  `mean_vpl_m` 1 ulp), so one platform always failed. Every field, key, metric order and
  line count stays exact; each metric may differ by at most 8 ulp, far below any engine
  revision.
- **D1.4 MRD: the fusion row's evidence class is read from the engine** (PR 6, merge
  `db2210a`; `c28e71a`). On engine 0.32.0 the "GNSS/INS sensor fusion" verification row is
  `ExternalDataset` / VALIDATED (the loosely coupled EKF against NaveGo on a real IMU/GNSS
  drive, GNSS present throughout), but the generated footnote still called it MODELLED.
  The footnote now takes the row's live status (`mrd::FUSION_ROW`), and two tests pin it
  to the table and its "no outage or coast is tested" caveat to the engine's own oracle
  text. The hand-written D1.4 banner, section 2 and section 4 were corrected: coasting
  through an outage is not validated and MRD-INT-04 stays partner / follow-on. The
  salvaged MRD draft was reconciled (nothing taken; a note records why).
- **Local-runner act image** (PR 6). `site-next/scratch/actlocal/image/Dockerfile`, which
  `rerun.sh` builds and which was never committed, is recreated: the medium
  `catthehacker/ubuntu:act-latest` image plus rsync and rustup with the pinned 1.93.0
  toolchain, for arm64 and amd64 (amd64 build verified; arm64 not yet). The stale
  `.rerun-snapshot-*.sh` copies were removed and are now ignored.
- **GNSS IQ layer partner deliverables** (PR 7, merge `78ab489`; `220675d`, `dbc8c81`,
  `4c7b6e2`). An executed GNSS IQ notebook against the published kshana 0.32.0 wheel
  (`examples/iq-notebook/`, with its builder and pinned requirements), and a one-page IQ
  layer validation summary for partners (`commercial/partners/`, Markdown and printable
  HTML with its renderer). The summary records the gps-sdr-sim ledger row as MODELLED by
  pre-registration and carries the hand-off caveat under its limits.
- **Handover** (PR 8, merge `1ca4c90`; `ba27e95`, and `eee0fc9` in PR 5). The post-0.32
  handover records the 0.32.0 Pro refresh, the partner deliverables, the account
  migration and recovery, and the session roster.
- **Changelog guard.** `tests/changelog_tracks_engine_pin.rs` fails unless this file names
  the `kshana` engine version pinned in `Cargo.toml` (as "engine X.Y.Z") in its
  `[Unreleased]` section or its newest release, so a pin bump cannot land unrecorded again.

## [0.2.0] - 2026-10-01

Kshana Pro 0.2.0, on open engine 0.29.0. Summary of this release (each item has its own
entry below):

- **Six power features.** The design optimiser (`kshana-pro design`: the Pareto front of a
  design space over any open kind); uncertainty and sensitivity at scale
  (`kshana-pro uncertainty`: confidence bands, the probability of meeting a limit, Sobol'
  indices, checkpoint and resume); the mission dossier (`kshana-pro mission build` and
  `mission check`: requirements to verdicts, PDF and HTML, a hash-checked evidence pack
  and a re-run check); campaign watch (`kshana-pro ci` over campaigns, a hash-chained
  history, trend pages and `ci diff`); spectrum coexistence (`kshana-pro coexist`, kind
  `pro-spectrum-coexistence`); and the on-premises job service (`kshana-pro serve
  --data-dir`: queued jobs that survive a restart, stored artefacts, a ledger and an
  OpenAPI description).
- **Model-based systems engineering (MBSE):** SysML v2 text, a trace, a verification
  cross-reference matrix, change impact and an evidence pack over real open-engine runs.
- **The MODELLED label on every output.** Every Pro result document and command output
  carries a `label` that starts with MODELLED, held by `tests/modelled_label.rs`.
- **The Ashforde document style.** Every HTML and PDF output uses the Ashforde theme, the
  registered seal (unaltered) and a seal watermark; `--sample` marks public samples
  "SAMPLE: illustrative programme on public data, not a real mission".
- **Open engine 0.29.0.** The development pin moves from the 0.28.0-reporting integration
  tree (`da55ae6`) to the 0.29.0 release tree (`feat/integration-bc` `38feb76`, 75
  scenario kinds): `kshana = { path = "../kshana-wt-pro-engine", version = "0.29" }`.
  After the crates.io publish the pin becomes `kshana = "0.29.0"`. No overlay source
  needed changing to compile.
- **Revision to published hashes and version stamps (see the next entry).**

- **Revision to published hashes and version stamps: Kshana Pro 0.2.0 on open engine
  0.29.0 (no verdict, count or figure at its published precision changed).** Every
  public showcase was run again with the development build of this branch at 0.2.0 on
  engine 0.29.0, with the same inputs, commands and `--utc` stamps, and `--sample` (the
  Ashforde SAMPLE marking). Every output now states engine 0.29.0 and overlay 0.2.0, so
  hashes over those documents changed:
  - Mission dossier: dossier SHA-256
    `c37799f792d6e47fe78ad57d2a7b91164272c8603b09f443b73206256c4eea03` →
    `49df1e0e78e07cc53649ff7d2ded07a53c02da1af508fce46660cb5da599eac9`; evidence pack
    `212686add4abbc7772b9d755574fa327822d7a4e1c8fb6c90916e3c44389d8b9` →
    `65d40cb458d2fb39d08c2ed8e890b4848ebe51df698e542e3bd686997750237a`; trade-study result
    `82e0ab74ea88d9f5526cc171581191cfbf958bac80d662ad705f7068ca21b782` →
    `a645049be5743f74f489920239964c0ae025e50ee532d263f3611c3ae6c3154d`. The low-Earth-orbit,
    campaign and L-band run results changed with the engine (the lunar and Mars ones are
    byte-identical); LEO-003's value moved from 0.43488783767 m to 0.43488821078 m (0.4349 m
    rounded, verdict unchanged). `mission check --rerun` repeated the six runs to the same
    bytes, and the open engine 0.29.0 release binary gave the same result and `report.json`
    bytes for all five open runs. Still 7 met, 4 not met, 1 not verified, 15 open items,
    21 pages.
  - Uncertainty run digests: clock holdover
    `159d2e1d37b4443bcb58bc8f068643478cac2c0fb9dc39e02d8ba8b1294d4138` →
    `abab765e36ce4b78865b1d9f91aa8240b683f22892abc5fbe9c58267af87ffbb`; low-Earth-orbit
    chain `2f2c4572d8dc085df3eca2f552fdca71a77e947cd41fc79278a36925ba417b24` →
    `fb54eac6f964fe134457ffe6fcc6a8294ec2ff0fa06af620fac2df71959aabb0`. Stopped at 700 of
    2,000 and resumed, the clock study still ends with the same six files as a run
    without a stop; the X-band link-margin study gives the same figures.
  - Campaign watch: history last hash
    `71397c3eee5b8822cf4b2109e135576fda9fef44c63201d049835d48dac67c2c` →
    `bb2f10ebaec45beb1ed95c4a54c652d351a2d9eda3c24d421860b30c2700dc4c`; every scenario hash,
    verdict and exit code is the same (r4-stronger-jammer still exits 1).
  - Unchanged: both lunar design run digests (`df81aad4…`, `53dcc9c4…`), the whole-Moon
    knee result (`89882372…`), all 43 front designs re-run by the open 0.29.0 release
    binary to the SHA-256 in their index (12 of 12 and 9 of 9 for the two lunar studies),
    and the coexistence scenario hash (`479364bd…`).
  - Job service: re-recorded; job identifiers, the constellation result and the counts are
    unchanged, the times, ledger hashes and the campaign job's run digest changed. In the
    crash-restart test the kill (0.3 s after three submissions) now lands while the second
    job runs, not the third (seen twice; a timing difference of this build): it
    ran again on its second attempt, the finished job was not run again, the queued one
    ran once, and the ledger verified 9 records and 3 delivered jobs.
  - Tests: the MBSE golden SysML and the D1.4 requirements table print Kshana Pro 0.2.0;
    the campaign-watch branch-point test also puts back the fixture's engine version
    (0.28.0) before its byte comparison.

- **Every Pro output is labelled MODELLED.** The public Kshana Pro page says so; now it is
  true. Each result document carries a top-level `label` string that starts with
  MODELLED, the open engine's own field form, and never leads with a higher tier.
  - Pro kinds that had no label now have one: `pro-trade-study`, `pro-batch`, `pro-mbse`,
    `pro-quantum-systematics`, `pro-lunar-signal-security`, `pro-beacon-dop`,
    `pro-surface-beacon-dop-study`, `pro-beacon-placement` and `pro-seam-check`. The seam
    check's optional scenario `label` is now echoed as `scenario_label`.
  - Four study labels that began with a description now begin with MODELLED
    (`pro-lunar-time-budget-study`, `pro-realtime-frame-eop-study`,
    `pro-optical-rf-hybrid-study`, `pro-cislunar-observability-study`); their text is
    otherwise unchanged. Rows tagged Validated inside the beacon and signal-security packs
    keep their tags.
  - Command outputs: the CI report (`ci --json`), each history record, the trend JSON,
    `ci diff --json`, the consolidated report JSON (`report --format json`), the study
    dossier JSON, the MBSE trace, verification cross-reference matrix (VCRM), change
    impact and evidence pack, and the mission dossier JSON. A CI report or history record
    written before the label reads back unchanged; older history chains still verify.
  - A job of the service now reads `evidence_tier` MODELLED for every Pro kind and for
    `ci` and dossier jobs; the job board and docs/SERVICE.md say so.
  - `tests/modelled_label.rs` runs one example of every registered Pro kind (and fails if
    a kind has none) plus each command output; the mission dossier and service tests
    check theirs. Each label site was removed once and its test went red.
  - **Revision to published hashes (no figure, verdict or run changed).** Mission dossier
    showcase, same command and `--utc`: dossier SHA-256
    `ddfb1599b7c20aec0db3f6e0aa0c2eab2f377c3e50174f7883634206e4b49e6d` →
    `c37799f792d6e47fe78ad57d2a7b91164272c8603b09f443b73206256c4eea03`; evidence pack
    `719bfbdf1d07f6078ca3bfb076bbaeec2f530315c024ce3755fbbbd011b21073` →
    `212686add4abbc7772b9d755574fa327822d7a4e1c8fb6c90916e3c44389d8b9`; trade-study result
    `e5f94602479a5d9f859b560e76687535772aefcb4cdeb627748cffd4ec90d425` →
    `82e0ab74ea88d9f5526cc171581191cfbf958bac80d662ad705f7068ca21b782`. Campaign-watch
    showcase, same commands and `--utc`: history last hash
    `f17496625f4536b1ea914bbbb5f031d1e153d28ccc57e232bacd174c8ee37177` →
    `71397c3eee5b8822cf4b2109e135576fda9fef44c63201d049835d48dac67c2c`, and every `ci.json`,
    trend and diff there gains the label. The branch-point compatibility test now allows
    exactly the one added label line. The design, uncertainty and coexistence showcases
    were run again and are byte-identical. The service showcase was recorded again at
    90b09f7 because its served job board showed the old `ci` text: every job result and
    artefact is byte-identical; the time stamps, the ledger hashes over them (ledger
    SHA-256 `6a155762ab945c3f…` → `bea4419a2d02cb1b…`) and the board text changed.

- **The clock twin result is labelled MODELLED.** The public Kshana Pro page says every Pro
  output is labelled MODELLED; a `pro-twin` result carried no label at all. It now has a
  `label` field that starts with MODELLED and says the twin reproduces a published
  Allan-deviation budget with the open clock model, not a measured unit, and its run
  summary ends `| MODELLED`. A unit test holds both. No figure changes. The trade-study
  result and the scenario regression check's JSON still carry no label (the regression
  check's HTML report does); labelling the trade-study result changes the bytes of the
  mission dossier's trade run and so the published dossier hash, which needs its own
  regeneration.

- **The three Pro power features integrated on `feat/pro-power`: design optimiser,
  uncertainty and sensitivity, and mission dossier.** Each was merged from its own branch
  after an independent verification. The README count words each branch wrote
  (seventeen packs, twenty integration tests) are superseded by the merged state: the
  registry holds **eighteen** Pro packs (`pro-design-optimise` and `pro-uncertainty` are
  new; the mission dossier is a command, not a scenario kind) and `tests/` holds
  **twenty-two** integration tests (`design_optimise.rs`, `uncertainty.rs`,
  `mission_dossier.rs` are new). The `kshana-pro --help` text now lists the `design`,
  `uncertainty` and `mission` commands (they ran before but were not listed). The README
  status tables name the two new packs and the `mission` module, and `docs/PRO-POWER.md`
  summarises all three features and what is not done. No published number changes.

- **Pro now builds on the integrated open engine: 75 open scenario kinds (was 63), all
  reachable through Pro. DEVELOPMENT PIN, not a release pin.** On branch `feat/pro-power`
  the `kshana` dependency points by path at a dedicated, read-only engine worktree
  (`../kshana-wt-pro-engine`, branch `feat/pro-engine-base` of the open repository, commit
  `da55ae6`). That engine adds twelve kinds to the published 0.28.0: `spectrum`,
  `solar-system`, `body-pnt`, `constellation-design`, `campaign`, `leo-signal`,
  `leo-pass`, `leo-navmsg`, `leo-pvt`, `leo-ppp`, `ntn-positioning` and `leo-pnt-chain`
  (spectrum and waterfall, the solar system and positioning around any body,
  constellation design, campaigns, and the low-Earth-orbit positioning, navigation and
  timing (LEO-PNT) chain; both lists were counted with each binary's `kinds --json`). The
  engine's animation, report and interoperability-export commands are command-line
  features of the open binary, not scenario kinds, and are not re-exposed by the
  `kshana-pro` command line here. The tree still reports version 0.28.0 but is
  not the 0.28.0 on crates.io. **Release step owed, not done:** before any Pro release,
  replace the path with the published engine version that carries these kinds, run
  `cargo update -p kshana`, and re-run the whole check set with `--locked`; a path pin
  builds only on a machine that has the worktree. No overlay source needed changing to
  compile: the engine's public seams (`api`, `registry`, `fom_label`, the model traits)
  are unchanged for the overlay's uses. New: `kshana-pro kinds --json` prints the open
  and Pro catalogues with `open_count` and `pro_count` (the open half is read from the
  embedded engine, never re-listed), and `tests/open_engine_reachable.rs` runs every
  open kind from one of the engine's own generic scenario files three ways (open engine,
  Pro dispatcher with no licence, open ∪ Pro registry by id) and requires the same result
  document byte for byte. It never uses the engine's optional Celeste preset files, so it
  passes with that preset absent. The README states nineteen integration tests (was
  eighteen). The sixteen Pro packs are unchanged and all still build and run
  (`registry::tests::every_pro_kind_builds_and_runs_through_the_registry`).
  Development and test builds now compile the embedded engine at optimisation level 3
  (`[profile.dev.package.kshana]`), so the library tests take about half a minute instead
  of about nine. Measured on the engine's 138 scenario files (its optional preset's five
  included) against the open release binary: `kshana-pro run` from a release build gave
  the same result document byte for byte on all 138; from a development build it matched
  136 and differed in the last bits of one field in each of two files
  (`combined-altpnt`, `gravity-map-nav`; relative 3e-15 to 6e-15). A number for a
  customer or a publication comes from a release build.

- **Model-based systems engineering (MBSE) and programme tooling over the open engine's
  own outputs (`kshana-pro mbse import|trace|vcrm|sysml|impact|pack`).** Requirements are
  imported from comma-separated values (CSV), a documented subset of the Requirements
  Interchange Format (ReqIF 1.2) and Systems Modeling Language v2 (SysML v2) text. Each
  carries a verification method (test, analysis, inspection or demonstration) and an
  acceptance criterion: a machine-checkable expression over the fields of an open-engine
  `result.json`, with units, aggregates (`max`, `p95`, `fraction`, ...) and `for all` /
  `for any` quantifiers over arrays. A trace file links requirements to runs; each
  criterion is evaluated against the real result document and unit-checked through the
  engine's field-units schema (`kinds.<kind>.fields.<path>.unit`), with the result's own
  `units` block as fallback, and a unit mismatch is reported as not verified. The
  verification cross-reference matrix (VCRM) records, per requirement and run, the
  verdict (met / not met / not verified), the run's `scenario_hash`, `seed`,
  `engine_version` and `schema_version`, every field path and value read, and the
  VALIDATED / MODELLED / PARTNER label the verdict rests on (from `figure_tiers`, the
  schema's `evidence_tier`, the report's `capabilities.rows` and the verification
  matrix), so a MODELLED or PARTNER basis is flagged. A scenario or result file whose
  SHA-256 no longer matches its report, or a scenario file whose `kind` or `seed`
  disagrees with the run's, makes the run's verdicts not verified. The SysML
  v2 export (part definitions for the simulated clocks, receivers, constellations and
  jammers with units from the schema, requirement definitions, verification cases per
  run, `satisfy` only where met) passes an in-repository checker for the grammar subset
  it uses, documented in `docs/MBSE.md`. Change impact lists the requirements to
  re-verify between two traces (scenario hash, engine version, seed or capability label
  changed, or, when no stamp tells the runs apart, the result file's SHA-256) or two
  scenario files (keys compared after parsing). The evidence pack is one
  offline HTML page plus JSON with the VCRM, a traceability graph, the reproducibility
  record of every run, open items and a sign-off table, hashed for reproducibility. The
  commands need the `pro-mbse` feature (Enterprise tier), gated exactly like `ci` and
  `study`. Tests: `tests/mbse.rs` (22 tests) over real `kshana 0.28.0` runs of
  clock-holdover, integrity-raim, campaign-jam-spoof-holdover-integrity,
  leo-pnt-end-to-end and a re-specified clock-holdover, committed with their provenance
  in `tests/fixtures/mbse/`, plus unit tests in each new module. The README states
  eighteen integration tests (was seventeen).

- **Study dossier (`kshana-pro study --dossier`), and the README pack count fixed.**
  One scenario now becomes a reproducible evidence dossier, written as JSON
  (`dossier.json`), a self-contained HTML page (`dossier.html`) and the exact scenario
  text run (`scenario.run.toml`). The page is laid out like a European Space Agency
  (ESA) technical note: identification, scope, applicable and reference documents,
  method, results, limitations. It records the open engine and Pro versions, the SHA-256
  of the input, the engine's scenario hash, and a SHA-256 result hash over the whole
  dossier; the same scenario gives the same hash and byte-identical files (an optional
  `--utc` issue stamp is kept outside the hash). Every figure carries the VALIDATED or
  MODELLED tier the open verification matrix grants it, read through
  `kshana::fom_label::tier_for`; a figure with no owning row is MODELLED. A
  one-at-a-time sensitivity sweep re-runs the scenario with each key parameter changed
  alone and reports each figure's change and elasticity. For `clock` scenarios the
  dossier adds the Pro holdover band and its maximum time interval error (MTIE) check
  against the scenario's own time-error budget and the published clock-class limits
  already in `twin::holdover_uq`: the enhanced primary reference time clock (ePRTC) 30 ns
  and primary reference time clock (PRTC-A) 100 ns class limits, and the conventional
  1.5 µs / 24 h holdover budget (an industry convention, not an ITU-T clock-class limit). No new mask values
  were added: the MTIE check already existed. The run's behaviour is read against the
  four levels of the US Department of Homeland Security (DHS) Resilient PNT Conformance
  Framework (RPCF) v2.0, Table 1, with each of its eight minimum requirements recorded
  as not assessed where a simulation cannot verify it, and Prevent / Respond / Recover
  readings are offered for later mapping to IEEE P1952, whose draft is not public. All
  of it is labelled "tentative, simulation-derived, not a certified level". The Goal
  Structuring Notation (GSN) argument, Verification Control Document (VCD) and W3C-PROV
  record are built with the existing L3 types; a `pro-trade-study` scenario also gets
  the trade's own gate, GSN, VCD and provenance appended unchanged. The command needs
  the Pro tier (`pro-study-dossier`) and fails closed without a licence. Worked example:
  `scenarios/dossier-holdover-csac.toml`. The README said seven Pro packs; the registry
  holds sixteen, and the README and `docs/OVERLAY-ARCHITECTURE.md` now say so. The new
  `tests/readme_counts.rs` fails if either count, or the README's integration-test
  count, stops matching the registry or `tests/`. Tests: `tests/study_dossier.rs`
  (determinism, tier split, the binary end to end and its licence gate) and unit tests
  in `src/study_dossier.rs`.

- **Design optimiser: the Pareto front of a design space, over any open kind and around
  any body (`kshana-pro design <study.toml> --out <dir>`, new pack `pro-design-optimise`).**
  A study states variables written into a base scenario of any open kind, constraints
  (a Walker pattern constraint that removes infeasible designs without a run, and conditions
  on result fields that keep a run off the front) and two or three objectives, each a field
  of the engine's result or a design variable; there is no cost model. Every design is one
  run of the open engine through `kshana::api::run_scenario`; the front and its knee come
  from the open kernel `kshana::mcda::pareto`. The search is exhaustive when the feasible
  space fits the budget and a seeded NSGA-II (non-dominated sorting genetic algorithm II)
  otherwise; a front that is not exhaustive is labelled "best found in N runs". The same
  study and seed give the same bytes for any thread count. Outputs: `design.result.json`
  (with the Pro result stamp: `scenario_hash`, `units` for every numeric field,
  `figure_tiers`, `not_modelled`, `reproducibility.run_digest`), `design.pareto.svg`,
  a self-contained `design.report.html`, `front/index.csv` and `front/NNN.toml`, each front
  design as a plain open scenario that the free engine runs to the same result document.
  Every result is labelled MODELLED, and each figure takes the label of the weakest open
  capability it rests on. Five worked studies in `scenarios/pro-power/design/` (a lunar
  navigation constellation over the whole Moon and over the south polar cap, a low-Earth-orbit
  positioning shell, a Mars shell, a link budget), with provenance. Measured: the 608-design
  lunar space has a front of 12 designs at 6 points (8 satellites 19.5446 % availability
  to 24 satellites 100 %), equal to a brute-force front over scenarios the test writes
  itself; through the evaluator the GPS (Global Positioning System) baseline constellation
  reads the published horizontal dilution of precision (median 0.94, 95th percentile 1.255
  against 1.25 published); on the analytic ZDT1 benchmark the search kernel's mean distance
  to the true front is 0.00018 against a bar of 0.05 fixed beforehand (the first run missed
  the bar at 0.177 because the crossover lacked its exchange step; the operator was
  corrected and the bar was not moved). The feature falls to the default, fail-closed
  licence tier. Guide: `docs/DESIGN-OPTIMISER.md`. Tests: `tests/design_optimise.rs`
  (the README now states twenty integration tests and seventeen Pro packs).

- **Design optimiser: five defects found by an independent verification, each fixed with a
  test.** (1) A key the study format does not have was ignored, so a misspelt table
  (`[[constraints]]` for `[[constraint]]`) dropped the constraint and the study still
  returned a front; an unknown key is now an error at every level, on `kshana-pro design`
  and on `kshana-pro run`, and a file of another `kind` is refused. (2) A `unit` written in
  the study file replaced the unit the engine's own result states, so `unit = "m"` could
  relabel a number the engine gives in kilometres; the engine's unit is now always used,
  the study's only where the engine states none (as the guide already said), and the
  `units` note records a disagreement. (3) `kshana-pro design --out <dir>` deleted the
  whole `<dir>/front/` folder before writing; it now removes only `index.csv` and
  `NNN.toml` files of an earlier run. (4) The report and the chart rounded to four
  decimals, so a value of 0.000004 read "0"; a number below 0.1 now keeps four significant
  digits. (5) The README still said sixteen packs in one sentence; it says seventeen and
  `tests/readme_counts.rs` now reads that sentence too. None of this changes a published
  number: the five worked studies give the same `design.result.json`, chart, report and
  front files byte for byte, with the same study hashes. The test that keeps the optional
  low-Earth-orbit preset out of the feature's scenarios now looks at file names inside the
  scenario folder, not at the path the repository is checked out in.

- **Uncertainty and sensitivity at scale, over any scenario kind (`kshana-pro
  uncertainty`, kind `pro-uncertainty`).** A study names any open scenario, states how
  uncertain its inputs are (normal, uniform, lognormal, triangular or discrete, each with
  the source of that uncertainty; the word `assumed` is allowed and is then printed as
  ASSUMED on every output) and lists the outputs to watch. It runs the open engine over a
  seeded Latin hypercube or plain random sample and reports, per output: the mean, the
  standard deviation, the 5th, 50th and 95th percentiles and a bootstrap interval on the
  mean, the running estimate as runs accumulate, the probability of meeting a limit with
  a Wilson interval ("given the stated input uncertainty"), and for a time series the
  percentile band at every step. Sensitivity is either standardised regression
  coefficients from the same runs, flagged in words as not reliable when the R-squared of
  the linear fit is below 0.7, or first-order (Saltelli and others 2010) and total-order
  (Jansen 1999) Sobol' indices with bootstrap intervals, from `n * (k + 2)` runs for `k`
  inputs. A constant output or input gives null with the reason, never zero. The result
  is byte-identical for any thread count. The command appends one line per finished
  realisation to `runs.jsonl`, stops cleanly at `--max-runs` and continues with
  `--resume`, refuses a study that changed since the checkpoint was started, and repairs
  a last line cut short by re-running that realisation; a stopped and resumed study ends
  with the same six files as an uninterrupted one. A failed engine run is recorded and
  counted, never dropped, and never counts as meeting a limit. With no input and the
  engine seed varied, a study reproduces the open `campaign [monte_carlo]` bit for bit
  (samples, statistics and bootstrap interval), so free and Pro never disagree on a
  number. Every result is labelled MODELLED, carries a unit for every numeric field and
  the base kind's tier for every output, never a higher one, and lists what is not
  modelled (correlation between inputs first). `pro-batch` is unchanged. Needs the
  Enterprise tier (feature `pro-uncertainty`); fails closed without a licence. New:
  `src/batch/uq/`, `src/packs/uncertainty_pack.rs`, five studies and their engine
  scenario copies with provenance in `scenarios/pro-power/uncertainty/`, the guide
  `docs/UNCERTAINTY.md`. Tests: `tests/uncertainty.rs` (15 tests: the Ishigami closed
  form, a link margin's closed form, the open campaign reproduced, the mixture closed
  form for an uncertain clock noise level, the low-Earth-orbit navigation chain against
  direct engine runs, thread-count independence, stop and resume, failed runs,
  adversarial studies, the licence gate) and unit tests in each module. The README now
  states seventeen Pro packs (was sixteen) and twenty integration tests (was nineteen).
  Hardened after an independent review: a declared output `unit` that differs from the
  unit the base kind reports is refused (it was taken as given); a distribution that
  gives a value that is not a finite number is refused before anything runs; a study
  whose every realisation fails finishes with null and a reason under `src` (it
  panicked); one study makes at most 10,000,000 engine runs (a very large `n` exhausted
  memory); a new study does not start beside the result files of another run. No
  published number changes.

- **Mission dossier (`kshana-pro mission build` and `mission check`): requirements to
  runs to verification matrix to a branded PDF, in one command.** A programme file
  (TOML: title, reference, issue, classification, a requirements file in comma-separated
  values (CSV), Requirements Interchange Format (ReqIF) or Systems Modeling Language
  version 2 (SysML v2), a list of scenarios of any kind with optional headline figures
  and charts, optional links, an optional `pro-trade-study`, sign-off rows) becomes one
  deliverable. Every scenario's exact bytes are run through the gated dispatcher and
  written under `runs/<id>/` as the open engine's own command line writes them, with the
  open engine's report (`kshana::advanced_report::build`), so the requirements tooling's
  integrity checks hold; a Pro kind is written without a report and the dossier says its
  integrity rests on the result hash alone. The trace file is written and the
  verification cross-reference matrix and evidence pack are built by the existing
  model-based systems engineering (MBSE) code (`mbse::trace`, `mbse::pack`); no second
  requirements engine was written. Outputs: `dossier.json` (with `dossier_sha256` over
  the canonical body; the `--utc` stamp is outside the hash), `dossier.html`
  (self-contained, no network, print stylesheet), `dossier.pdf` (A4, brand colours,
  cover, the MODELLED banner with "not a certification", tables with repeated headers,
  vector charts, page numbers), `runs/`, `trace.toml`, the evidence pack, `manifest.json`
  and a hash-chained `ledger.json` (`study_ledger`). A figure is only a scalar the
  result's own `units` block marks `provenance = "computed"`; inputs and constants are
  listed as inputs. `mission check <dir>` re-hashes every file and names what changed;
  `--rerun` runs every stored scenario again and compares. Needs the
  `pro-mission-dossier` feature (Enterprise tier, or granted by name); a Pro kind in a
  programme still needs its own tier. Worked example in `scenarios/pro-power/mission/`:
  five open-engine runs on public data (a low Earth orbit navigation chain, a lunar
  constellation, the L band under interference, a chained campaign, Mars orbit
  navigation) and one trade study; twelve requirements give 7 met (all 7 on a MODELLED
  or PARTNER basis), 4 not met and 1 not verified, with 15 open items. Checked against
  the free engine: with the open release binary all five result files and reports are
  byte-identical to the dossier's; with an unoptimised debug build of the open engine
  four are, and the low Earth orbit chain differs in 33 floating-point fields by a
  relative 4.8e-5 at most, with no verdict changed. Guide: `docs/MISSION-DOSSIER.md`.
  Tests: `tests/mission_dossier.rs` (every expected value is also read from a direct
  open-engine run) and unit tests in `src/mission/`. The README states twenty
  integration tests (was nineteen). **Two revisions of existing output.**
  (1) `kshana-pro study --dossier`: when a result's `units` block carries provenance,
  the default figures are only the `computed` fields, so for kinds other than `clock`
  echoed inputs and constants are no longer listed as figures (measured on four
  scenarios: lunar constellation 30 figures to 18, low Earth orbit chain 33 to 16,
  spectrum 25 to 7, Mars unchanged at 12). The clock worked example is unchanged: the
  dossier hash of `scenarios/dossier-holdover-csac.toml` is `d5e3acdc…9d3e` before and
  after, and its three files are byte-identical. (2) `kshana-pro report --pdf` and the
  Pack-A PDF: `report_pdf::Layout::fill_rect` drew the banner with `Op::DrawRectangle`,
  which `printpdf` 0.9.1 writes as a clipping path, so the banner background was never
  painted and everything after the banner on its page was clipped away; it now draws a
  filled polygon. The footer separator, a middle dot the built-in fonts showed as two
  wrong characters, is now a vertical bar. These PDFs now show their full first page;
  apart from that separator their text is unchanged.

- **Mission dossier and acceptance criteria: three refusals that protect a verdict.**
  (1) `kshana-pro mission build` refuses run identifiers that differ only in letter
  case, and any spelling of `TRADE` for a scenario. Each identifier becomes a directory
  under `runs/`; on a file system that ignores case (the default on macOS and Windows)
  `RUN-A` and `run-a` shared one directory, the second run overwrote the first one's
  files, and the build returned success with the first run's verdict read from the
  second run's result (measured on two Mars runs with seeds 7 and 99: a limit of 1.9 m
  that seed 7 meets at 1.8827 m was reported not met at 2.0928 m, and `mission check`
  then failed on its own dossier with 10 problems). (2) It refuses a scenario that
  holds a value that is not a finite number (`nan`, `inf`), naming the key, also inside
  a variant of a trade study: the engine takes such a value (measured on the open engine's
  `clock-holdover.toml`: the classical clock's holdover is 2610 s with the 20 ns
  threshold and reads 6600 s with `threshold_ns = nan`, which is never crossed), the
  result echoes it as `null`, and no sheet showed it. (3) **A revision of existing
  behaviour in `kshana-pro mbse`:** a criterion number must be finite. A literal too
  large for a double (`<= 1e999 m`) parsed to infinity and the criterion read as met
  for every value; it now does not parse, and the requirements file does not load.
  Tests: `run_ids_that_differ_only_in_letter_case_are_refused` and
  `values_that_are_not_finite_numbers_are_refused` in `tests/mission_dossier.rs`, and
  unit tests in `src/mission/programme.rs` and `src/mbse/criterion.rs`. Three more
  tests close gaps a mutation check found (the code was right, no test held it):
  `the_summary_counts_each_label_as_the_matrix_carries_it` (a PARTNER verdict counted as
  MODELLED went unnoticed), `markup_in_programme_text_is_escaped_in_the_html` (an
  unescaped page went unnoticed) and `a_ledger_from_another_dossier_fails_the_check`.
  **A changed number in `mission check`'s output:** the count of files re-hashed is
  now of distinct files. A file listed by both `dossier.json` and `manifest.json` was
  counted twice, so the worked example printed 65 for the 35 files it holds (30 of them
  listed twice); it now prints 35. Every comparison is still made.

- **Campaign watch: the scenario subscription (`kshana-pro ci`) now understands campaigns
  and statistics.** A pack manifest can carry `[[watch]]` tables: the figures that matter
  (a result path, an aggregate such as `max(...)`, a named campaign figure, or a sweep
  crossing), what worse means for each (`higher`, `lower` or `target`), a tolerance, an
  optional absolute requirement checked with units, and, for a Monte Carlo figure, the
  ensemble's mean, standard deviation and count. A watched pack collects figures only:
  numeric leaves whose units entry says the run produced them (on
  `l-band-waterfall-jamming`, 199 metrics become 112; the receiver noise figure and the
  waterfall grid size are gone). Campaign results yield named figures (each channel's
  value at the end of each phase, carried values, first alarm time, share under alarm,
  event times, Monte Carlo statistics, sweep extremes). A Monte Carlo watch is decided by
  a Welch t-test on the means and an F-test on the variances at a stated alpha (default
  0.01): 200 other seeds of the clock ensemble move the mean by 156 % and are not a
  regression (p = 0.65); a doubled spread is (F = 4.0, p = 2.4e-21). New flags:
  `--history` (an append-only, hash-chained `history.jsonl`; `kshana-pro ci history
  verify` names the first broken record), `--trend` (an HTML page with a sparkline per
  watched figure, and JSON), `--exact` (report any changed member result SHA-256), and
  `kshana-pro ci diff old.json new.json`. Five subscription packs ship under
  `scenarios/packs/` (low-Earth-orbit, constellations, spectrum, campaigns, the solar
  system), each with provenance, plus a six-revision demonstration sequence under
  `scenarios/pro-power/watch-demo/`. A pack without watch tables runs exactly as before:
  same pack hash, same report, byte for byte (tested against the branch-point report).
  Guide: `docs/CAMPAIGN-WATCH.md`. The README states twenty integration tests (was
  nineteen).

- **Spectrum coexistence (new Pro power track, feature and kind `pro-spectrum-coexistence`).**
  `kshana-pro coexist <study.toml> --out <dir>` tests whole candidate signal plans (a signal
  design plus a constellation) against every open-service global navigation satellite
  system (GNSS) signal the open engine models (GPS L1 C/A and L5; Galileo E1, E5a, E5b and
  the E5 alternative binary offset carrier (AltBOC) signal), in both directions, over a
  worldwide grid, and gives a verdict per plan and signal against the study's own
  `max_degradation_db` and `limit_source` (both required; no limit is built in). Method:
  effective carrier-to-noise density C/(N0 + I) (International Telecommunication Union
  Recommendation ITU-R M.1831; Betz 2001; Kaplan and Hegarty section 9.4). Every physical
  number is read from open `leo-signal` and `constellation-design` runs; the pack adds only
  the aggregation and ranks plans with `kshana::mcda::pareto`. The result is labelled a
  MODELLED upper bound (every satellite in view at the maximum received power, no
  elevation-dependent power, no receive-antenna pattern), lists every open run with the
  SHA-256 of its scenario and result, and stops if its rebuilt single-satellite loss
  differs from the open run's by more than 1e-9 dB. Refused: any table named for jammers,
  spoofers, attacks or threats; any victim outside the open list (GPS L2C included: the
  open engine has no compatibility model for it), and any reference or maximum received
  power of 0 dBW (1 W) or more (an EIRP or a unit slip, not a received power). No
  jammer-tolerance field is carried through. Outputs: result JSON, matrix, map and Pareto charts (SVG), a self-contained HTML
  report; `kshana-pro run` prints the JSON. New: `src/spectrum_coexistence/`,
  `src/packs/spectrum_coexistence_pack.rs`, `scenarios/pro-power/coexistence/`,
  `tests/spectrum_coexistence.rs`, `docs/SPECTRUM-COEXISTENCE.md`. Seventeen Pro packs,
  twenty integration tests.

- **On-premises job service: queued runs that survive a restart, stored artefacts, a
  delivery ledger.** `kshana-pro serve --data-dir <dir>` keeps jobs on disk
  (`<dir>/jobs/<job_id>/`: request, status, result, artefacts; every write a temporary
  file renamed into place), runs them on a bounded worker pool (`--workers N`, default
  1), and on start queues again every job left queued or running, so a stop or a crash
  loses no work; a finished job is never re-run. The job identifier is the SHA-256 of the
  operation and the canonical JSON (JavaScript Object Notation) request, so the same
  request submitted twice is one job and one execution. New routes: `POST /jobs`,
  `GET /jobs`, `GET /jobs/<id>`, `GET /jobs/<id>/result` (409 until succeeded),
  `GET /jobs/<id>/artefacts/<name>`, `GET /ledger`, `GET /ledger/verify`,
  `GET /openapi.json` (OpenAPI 3.1, generated from the router's own route table) and a
  self-contained job board at `GET /`. Operations: `run` (any open or Pro kind; the
  result is the run's document byte for byte, with the chart and table as artefacts),
  `dossier` (the study dossier) and `ci` (a Pack C scenario pack with its scenario texts
  inline); the entitlement is checked at submission (403, nothing queued) and again at
  run time. Each succeeded job appends request, run and delivered records to a
  hash-chained ledger (`src/study_ledger.rs`); verification names the first changed
  record and also re-hashes every delivered job's stored files. A failed job carries the
  engine's error and is never recorded as delivered. Optional `--api-token-file`: every
  route except `/health` then needs a bearer token, compared in constant time over its
  SHA-256 and never logged or echoed. With a data directory, `POST /study` also queues a
  `dossier` job for `inputs.scenario` and adds `job_id`; without one every existing route
  behaves exactly as before. `POST /jobs` accepts bodies up to 16 MiB (mebibytes); 413
  above. No new crate dependency, no asynchronous runtime, no outbound connection. The
  compose files mount a data volume, and `deploy/DEPLOY.md` covers the routes, the data
  directory, backup and restore, and the token file. Guide: `docs/SERVICE.md`. Tests:
  `tests/service_jobs.rs` (19 tests over real sockets: the 5000-satellite design and the
  200-seed campaign byte-identical to direct runs, idempotence, restart recovery with a
  control that fails without recovery, entitlement, failure, hostile input, the OpenAPI
  document against the route table both ways, ledger tampering, the token, `POST /study`
  with and without a data directory, the outgoing-socket scan, three workers, the `ci`
  operation, the job board, and a guard that stops a spawned server when a test fails),
  on public engine scenarios recorded in
  `tests/fixtures/service/PROVENANCE.md`. The README states twenty integration tests
  (was nineteen).
