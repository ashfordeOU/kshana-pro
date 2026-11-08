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
| L-kind93/0001, 0002 | site at 93 kinds | (1) `web/` re-ported from a full site rebuild at open `ws/tip-and-cue-kind` 0a53cb87e (RE-PORT.md: engine, MCP, WebAssembly package, Studio re-record, every recorder, build.py, check.py), sources kshana-pro `site/desktop-studio` b2ed1332, plus `web/capabilities.json`'s proof line (93 of 93 kinds, 3334 of 3334 fields); the recorder-diff list is in its message. (2) `i18n/`: English page catalogue re-extracted, translations carried over digit-only changes, orphans dropped. Placements, the register state, the unplaced scenarios and a deploy-size proposal: `L-kind93/NOTES.md`. Base: 0a53cb87e (apply in order) |
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
