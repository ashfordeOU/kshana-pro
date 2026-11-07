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
| L3-station/0001 | L3 | Kshana Station: `src/station.rs` (`kshana station run|check|service`), `tests/station_service.rs`, `docs/STATION.md`, `examples/station/station.toml`, catalogue messages (`engine.ftl`, `manual.json`), allowances with reasons (`allow.json` 25, `sinks-allow.json` 4: HTTP lines, service templates, JSON bodies), `docs/COMMANDS.md` and `docs/SURFACES.md` rows. Base: open main 912fe04 |
| 0001 | L2a | Kshana Desktop (applied as a13bd6a): `apps/desktop` (Tauri 2), native runs through `src-tauri/src/bridge.rs`, `web/studio/lib/desktop.mjs` (+ node test and its CI step), a `desktop` workflow for Linux, macOS and Windows bundles (unsigned), a `docs/SURFACES.md` row |

L3 checked before export, on open main 912fe04: `cargo fmt --check`, `cargo clippy --all-targets
-D warnings`, `tests/station_service.rs` (4 tests over real sockets), `tests/i18n_catalogue.rs`,
`tests/i18n_ratchet.rs`, `tests/no_overclaims.rs`, `tests/cli_help_lists_subcommands.rs`, and
`scripts/check-doc-coverage.sh` (964, at the ceiling). Not run: the whole suite.

L2a checked before export: the bridge's Rust tests (a native run equals `kshana::api`, a
1 GiB recording probed in blocks), clippy and rustfmt of `apps/desktop/src-tauri`, the
node test, and a launch under a virtual display in which a scenario ran natively.
Nothing here is Pro code.
