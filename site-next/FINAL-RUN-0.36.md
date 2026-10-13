# 0.36.0 final site run: handoff

Written for whoever runs the final port if the previous runner is out of context. Facts first, then the steps.

## State (2026-10-13)
- Public repo branch `site-port/0.36.0`: release/0.36.0-rc merged in, plus the Studio port patch (Geometry and Lunar
  views), capability cards (sky geometry, geometry-aware trust, lunar reference station), the sky_* rows in
  docs/PYTHON_API.md and README.npm.md, the D5 card text (loss-of-lock standoff 19.4 km) and the wording fix in
  docs/LUNAR-REFERENCE-STATION.md ("at each user point").
- This repo branch `port/0.36.0`: the Studio from `site/geometry-0.36`, the skyline docs page and the lunar reference
  station page in tools/build_docs.mjs, the "New in" section keyed to the minor version (build.py NEW_SECTIONS),
  placements for the 0.36 features in features/placements.toml.
- Held until the coordinator sends the final rc SHA: version bump, record pass, counts, social card, push.

## Steps on the final SHA (about 45 minutes; the engine build is most of it)
1. Public repo: merge the final rc SHA into site-port/0.36.0.
2. Bump the version to 0.36.0 first (Cargo.toml, mcp/kshana-mcp, server.json, JetBrains, plugin manifests, CHANGELOG
   heading, CITATION.cff date-released, README anchor). `scripts/check-version-sync.sh` lists every surface.
3. Build: `/tmp/b35.sh` pattern = `cargo build --release --locked --bin kshana`, the same in mcp/kshana-mcp, then
   `./web/build.sh` (needs binaryen version_117 on PATH). About 18 minutes cold.
4. Studio record pass in this repo's Studio folder (one pass, after the bump so the records carry 0.36.0):
   `node tools_native.mjs --repo <kshana> --bin <kshana>/target/release/kshana --work /tmp/native-work --jobs 4`, then
   tools_trust_demo, tools_interference_demo, tools_training_demo, `tools_trust_geometry_demo.mjs --pkg <abs pkg>`,
   tools_record, tools_areas, tools_bundle, selfcheck (`--final` once committed). Copy pkg from web/pkg first.
5. Site recorders (src/tools/record_runs.py and the gen_* scripts, then build.py) only on a clean, pushed public SHA;
   build.py refuses recordings stamped at another commit. Use `KSHANA_DOCS_REMOTE_REF=origin/<pushed branch>` so the
   docs link to the post-merge tree (no "Not yet on GitHub" spans). `KSHANA_NEW_SECTION` is not needed after the bump.
6. check.py, features gen_features.py and check_features.py (only the ANIMATION census may still be red: regenerate it
   with `cargo test --test animation -- --nocapture` and edit docs/ANIMATION.md, a public doc).
7. Counts and artefacts from the final ledger: `cargo run --bin gen_validation_artifacts` (regenerates
   VERIFICATION-MATRIX.md, MODELLED-RATIONALE.md, web/data/verification-matrix.json), the figures and og-card
   (`gen_readme_assets.py`, `gen_validation_figures`), README counts, kind counts (75 -> the dispatcher's). The guards that
   name every stale site: the `*_doc_sync` cargo tests, run with `--no-fail-fast`.
8. Commit kshana-pro (clean tree) before porting so PORT-MANIFEST.json records dirty: false. Then in the public repo:
   `python3 web/tools/port_site.py --site <combo> --studio <studio-rebuild>` and `--check`, `./web/build.sh`, and the
   gates: site.test, legacy-urls, inline-json, lfs-pointer, test_port_site.py, all web/studio/lib tests, smoke,
   check-version-sync, check-no-attribution, the cargo doc-sync suites.
9. ONE re-port commit on top of the rc head, authored as Ashforde OÜ <ashfordegithub@cbaweja.org>, plain message.
   Pushing to the rc needs the owner-approved path; tell the coordinator when it is ready.

## Things that went wrong before
- Recorders stamped at a commit that was never pushed (squash after the stamp): run them on the SHA that is pushed.
- `gen_evidence_data.py` reports stale if the public working tree is dirty: run the recorders on a clean checkout.
- A stale remote ref makes build.py print "not-pushed" spans: fetch the ref as a tracking ref first.
- The site's price-wording rule rejects "per user" in any docs page.

## Gate: every 0.35 claim still holds on the final tree (owner's rule, added 2026-10-13)
0.36 must keep every claim marketed for 0.35. Before the final SHA is reported, check each claim below on the
final tree and put PASS or FAIL with the evidence (command and its output line) in the report. If anything
regressed, stop and report; do not paper over it.

| # | Claim | How to check (final tree, release build where stated) |
|---|---|---|
| 1 | Receiver trust scores every fix 0-100 from NMEA 0183 alone and names which checks reduced it and by how much | `cargo test --test receiver_trust_maritime --test receiver_trust_live --test receiver_trust_jammertest --test maritime_trust_reference`; `node web/smoke.mjs` (wasm `receiver_trust_replay` flags the drag-off, `assess_vessel_log` returns epochs) |
| 2 | Crew training: synthetic bridge NMEA with scripted jamming, position drag-off, time spoof and replay, plus the debrief view | `cargo test --test nmea_training_scenarios --test nmea_training_reference`; the Studio selfcheck's training demo (`tools_training_demo`) |
| 3 | Galileo OSNMA: all 18 official test-vector sets pass, 186,382 tags authenticated, 0 failures; unaffected by the WP-00 E1 table removal | the test is opt-in: download the ESA/GSC OSNMA test-vector archive, then `KSHANA_OSNMA_VECTORS=<dir containing osnma_test_vectors/ and cryptographic_material/> cargo test --release --test osnma_official_vectors -- --nocapture` and quote the sets / tags / failures line. If the archive cannot be fetched from this runner, report that plainly: the claim is then unchecked, not passed. Also `cargo test --test osnma_synthetic --test surface_osnma` (always runs) |
| 4a | Signed evidence packs verifiable offline | `cargo test --test evidence_pack --test evidence_cli --test evidence_crypto_reference --test evidence_assemble` |
| 4b | SOC telemetry: Prometheus, OpenTelemetry, CEF, LEEF | `cargo test --test telemetry_cli --test surface_trust_telemetry --test cef_reference` (find the LEEF and OpenTelemetry guards with `grep -l -i "leef\|otlp" tests/*.rs` and run them too) |
| 4c | Signal K and OpenCPN plugins | `scripts/check-version-sync.sh` (it names both plugin versions); the plugins' own tests under their folders |
| 4d | A per-day interference map from ADS-B and AIS | `cargo test --test interference_map_cli --test interference_map_examples --test interference_map_reference` |
| 4e | Compliance mapping by clause | `cargo test --test compliance_report --test compliance_references` |
| 4f | MCP tools at least 39 (state the actual count) | build `mcp/kshana-mcp` (release), send `{"jsonrpc":"2.0","id":1,"method":"tools/list"}` after `initialize` over stdio, count the tools, and compare with the number the README and `docs/MCP.md` state. Report the actual count |
| 5 | Validated count at least 132 and traceable in the public ledger | `web/data/verification-matrix.json` summary and `docs/VERIFICATION-MATRIX.md` (final ledger: 134 VALIDATED at 274 rows as of 12b42e4e; recount on the final tree) and `tests/verification_rows_cite_evidence_that_exists.rs` |
| 6 | kshana.dev runs in the browser with no install; installs via cargo, pip, npm, brew, scoop and the JetBrains plugin | `node web/smoke.mjs` and `node web/site.test.mjs` (browser build); the README install table lists cargo, pip, npm, brew, scoop and JetBrains. Registry availability of the new version is an outward fact only the owner can confirm after publishing: report it as such |

Counting rules for the report: use the tools' own output, never a number typed from a document; do not
substitute counts by pattern (the coordinator restored "275 epochs" after a blanket replace; "137 m @ 1-day arc",
"grid-resolution floor ~140 m" and "CCSDS 133.0" are not counts either).
