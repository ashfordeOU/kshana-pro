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
| 0001 | L2a | Kshana Desktop: `apps/desktop` (Tauri 2), native runs through `src-tauri/src/bridge.rs`, `web/studio/lib/desktop.mjs` (+ node test and its CI step), a `desktop` workflow for Linux, macOS and Windows bundles (unsigned), a `docs/SURFACES.md` row |

Checked before export: the bridge's Rust tests (a native run equals `kshana::api`, a
1 GiB recording probed in blocks), clippy and rustfmt of `apps/desktop/src-tauri`, the
node test, and a launch under a virtual display in which a scenario ran natively.
Nothing here is Pro code.
