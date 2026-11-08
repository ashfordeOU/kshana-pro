# Patches for the open repository (ashfordeOU/kshana)

The apps track's session could not push to the open repository (its access request was
refused), so the open-side commits are kept here as `git format-patch` files. Apply them
on the open repository's main and push from a session that has access:

```bash
cd kshana            # the open repository, on an up-to-date main
git am --committer-date-is-author-date /path/to/kshana-pro/handover/open-repo-patches/*.patch
git push origin main
```

| Patch | Item | What |
|---|---|---|
| L-headroom/0001 | deploy size | `web/` re-ported with the Studio's 184 recordings gzip-compressed (`studio/recorded/<name>.json.gz`, read through `lib/packs.mjs` with `DecompressionStream`): 43 MB to 6.9 MB, check.py 12b's deploy tree 498.1 MB to 458.1 MB. Sources kshana-pro `site/desktop-studio` 9983b3f3 (with the feature register's subcommand fix, 3aecd1ed). Checked: port --check, site.test, legacy-urls, smoke, every Studio module test, the 17 engine tests that read web/, the page catalogue unchanged, and Chromium with WebAssembly blocked (the Studio, six views and two site pages draw from the recordings). Base: open main b0ebfd86f |
| L-kind93/0001, 0002 | site at 93 kinds | (1) `web/` re-ported from a full site rebuild at open `ws/tip-and-cue-kind` 0a53cb87e (RE-PORT.md: engine, MCP, WebAssembly package, Studio re-record, every recorder, build.py, check.py), sources kshana-pro `site/desktop-studio` b2ed1332, plus `web/capabilities.json`'s proof line (93 of 93 kinds, 3334 of 3334 fields); the recorder-diff list is in its message. (2) `i18n/`: English page catalogue re-extracted, translations carried over digit-only changes, orphans dropped. Placements, the register state, the unplaced scenarios and a deploy-size proposal: `L-kind93/NOTES.md`. Base: 0a53cb87e (apply in order) |
| C7-3-naming/0001 | C7 sign-off | `docs/design/evidence/c7-flight-dynamics-adapters/NAMING.md`: the four tools with versions and licences, ANISE 0.10.6 named as the fourth before its adapter, and the reading of criterion 3 for a tool that converts units (the seat's ruling of 8 November). Docs only. Base: open main 228d118 |
| C7-2-orekit-basilisk/0001 | C7 steps 2–3 (apply after C7-1) | `integrations/flight-dynamics/`: Orekit (orekit_jpype 13.1.9.0, orekit-data pinned at 2d6edc91) and Basilisk (bsk 2.12.2, its own SPICE) adapters, notebooks (.md + .ipynb), the difference report, `run_notebooks.py`; `.github/workflows/flight-dynamics.yml` (connect check, CLI files, wheel, matrix of notebooks, CLI = Python bytes); `web/smoke.mjs` WASM digest check. Orekit carries OEM and OPM (every value = its km→m conversion of the written double, every epoch equal) and is named as rounding further (≤1 ulp); Basilisk reads 25,926 of 25,926 SPK values and 4,321 epochs bit for bit. Checked locally: both notebooks, the CLI byte comparison, `node web/smoke.mjs` on a fresh wasm-pack build, `integrations/check-doc-links.mjs`, `check-toolchain.sh`, YAML parse. Not run: the workflow itself (runs on apply). |
| C7-1-ephem-export/0001 | C7 step 1 | `kshana ephem-export` (`src/ephem_export.rs`; Python, WASM and MCP `ephem_export`, 53 MCP tools): a JPL Horizons state table written as a CCSDS OEM 2.0, an OPM 3.0 or an SPK of type 13 or 9, then read back by the engine's readers. CAPSTONE fixture: 4,321 of 4,321 states read back with the same bits (OEM, SPK; the OPM's one state); once outside the repo with CSPICE N0067, the SPK's records and epochs are the same bits and spkezr at the nodes is within 1.5e-11 km. The three files' SHA-256 are pinned and checked through the CLI, Python and MCP. `ReadSegment::nodes` added in `spk_writer.rs`. Docs: `docs/FLIGHT-DYNAMICS-EXPORT.md`, COMMANDS, SURFACES, README and MCP README tool tables, CHANGELOG. Checked on open main 185b665: fmt, clippy `--all-targets -D warnings`, `cargo check --target wasm32-unknown-unknown --features wasm`, `scripts/check-doc-coverage.sh` (964), i18n catalogue and both ratchets, readme completeness and code fences, cli help, no_overclaims, evidence labels, palette_sync, `gen_readme_blocks.py --check`, `web/site.test.mjs`, the new test, the MCP crate's fmt, clippy and round trip, and the Python binding test (maturin). Not done: `web/` re-port (the site's tool count and the new docs page come with the next port); translated READMEs still list the old tool count, as they already did. Base: open main 185b665 |
| L2-iq-view/0001 | L2b | IQ recording view in Kshana Desktop (`web/studio/iq/`, `lib/iqview.mjs` + test and CI step), re-ported from kshana-pro `site/desktop-studio` 71cac75 (site part unchanged, 8d10cf7); `port_site.py` and `site.test.mjs` list the new view; `studio-js.ftl` regenerated (36 messages). Read only: no write, transmit or signal-making control. Checked: `port_site.py --check`, `site.test.mjs`, `i18n.mjs check` (17 languages and en-XA, 0 errors), `extract` (identical), the lits, catalogues, digits, caps, migrate, renumber, size and first-paint tests, the module tests, and the page in Chromium with a fake Desktop over a synthetic 8 M-sample ci16 SigMF recording. Base: open main 7999be5 |
| L2-studio-port/0001 | L2a/L2b fix | `web/` re-ported with the Desktop Studio changes moved into the Studio source (kshana-pro `site/desktop-studio` 7bd2909): `PORT-MANIFEST.json` lists `studio/lib/desktop*.mjs`, `studio/lib/devices*.mjs` and the new `app.js` and `trust/trust.js`; `i18n/en/studio-js.ftl` regenerated with `tools/i18n.mjs jsx`; `web/tools/port_site.py` writes the Studio name into element text only (it overwrote the closing tag inside `data-l10n-tags`, the 'Studioundefined' bug fixed by hand in be569278e). Site input: the previous port's own site files (`--check` showed no difference outside the Studio). Checked: `port_site.py --check`, `web/site.test.mjs`, `i18n.mjs check`, `i18n-lits-test`, `i18n-catalogues-test`, the Studio module tests. Base: open main 9e59ff9 |
| L2-icons/0001 | L2a fix | `apps/desktop/src-tauri/icons/icon.ico` (16–256 px) and `icon.icns`, made from `icon.png`, listed in `tauri.conf.json` `bundle.icon`: the Windows job of `desktop.yml` failed in tauri-build without `icon.ico`. Base: open main 362d2a0 |
| L2b-devices/0001 | L2b | Live devices in Kshana Desktop: `apps/desktop/src-tauri/src/devices.rs` (port listing, read-only capture from a serial port, `tcp:<host>:<port>` or `file:<path>`, a rolling 1.5 MiB excerpt of NMEA sentences), `receiver_trust_replay` and `assess_vessel_log` in the native bridge, `web/studio/lib/devices.mjs` (+ node test and its CI step), a Desktop-only "Score a receiver live" panel on the trust page, English `studio-js.ftl` entries, README and `docs/SURFACES.md`. No engine (`src/`) change. Base: open main c8a3f7c |
| L3-station/0001 | L3 | Kshana Station: `src/station.rs` (`kshana station run|check|service`), `tests/station_service.rs`, `docs/STATION.md`, `examples/station/station.toml`, catalogue messages (`engine.ftl`, `manual.json`), allowances with reasons (`allow.json` 25, `sinks-allow.json` 4: HTTP lines, service templates, JSON bodies), `docs/COMMANDS.md` and `docs/SURFACES.md` rows. Base: open main 912fe04 |
| 0001 | L2a | Kshana Desktop (applied as a13bd6a): `apps/desktop` (Tauri 2), native runs through `src-tauri/src/bridge.rs`, `web/studio/lib/desktop.mjs` (+ node test and its CI step), a `desktop` workflow for Linux, macOS and Windows bundles (unsigned), a `docs/SURFACES.md` row |

L3 checked before export, on open main 912fe04: `cargo fmt --check`, `cargo clippy --all-targets
-D warnings`, `tests/station_service.rs` (4 tests over real sockets), `tests/i18n_catalogue.rs`,
`tests/i18n_ratchet.rs`, `tests/no_overclaims.rs`, `tests/cli_help_lists_subcommands.rs`, and
`scripts/check-doc-coverage.sh` (964, at the ceiling). Not run: the whole suite.

L2a checked before export: the bridge's Rust tests (a native run equals `kshana::api`, a
1 GiB recording probed in blocks), clippy and rustfmt of `apps/desktop/src-tauri`, the
node test, and a launch under a virtual display in which a scenario ran natively.
L2b checked before export, on open main c8a3f7c: `tests/evidence_labels_doc_sync.rs` and `tests/scenario_file_sources_guard.rs`, the Desktop crate's 6 Rust tests (ring and
sources; a growing file and a local TCP stream captured; the native scorer equal to
`kshana::surface::assess_vessel_excerpt` on the trust demo log), clippy `-D warnings` and
rustfmt of `apps/desktop/src-tauri`, `devices.test.mjs`, `desktop.test.mjs`,
`scripts/check-doc-coverage.sh` (964), and the trust page in Chromium with a fake Desktop
invoke (panel drives start, status, excerpt, scoring and stop; absent on the web page).
Not run: a real serial receiver; the Windows and macOS builds (the `desktop` workflow
builds them).

Nothing here is Pro code.
