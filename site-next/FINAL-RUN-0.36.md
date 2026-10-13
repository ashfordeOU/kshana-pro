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
