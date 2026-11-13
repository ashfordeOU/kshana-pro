<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->
# Kshana Desktop Pro

The free Kshana Desktop (the open repository's `apps/desktop`) with the Pro overlay linked
in (roadmap F10e). A separate download: the free build carries no Pro code
(`scripts/free-build-check.sh` checks a free binary).

- **Engine bridge.** The free Desktop's command names (`engine_call`,
  `engine_native_fns`), so the Studio's Desktop client is unchanged. A scenario of a Pro
  kind runs through `kshana_pro::registry::run_gated` under the Desktop licence; every
  other scenario on the open engine. `list_kinds` adds the Pro kinds the licence runs.
- **Pro home and licence screen** (0.42 U3, concept D; `pro/desktop/`). With the open Desktop
  shell in the staged checkout (`apps/desktop/shell`, 0.42 U1), `build-frontend.sh` stages it
  unchanged as `studio/desktop/` and adds the Pro overlay (`pro/desktop/overlay.mjs`,
  `pro.css`) to the staged copy of its `index.html`: the window opens the shell, whose Home is
  the Pro home ("What do you need to prove today?": the four jobs, the session's recent runs,
  the Pro products, the licence card). The licence screen is a Pro page in the shell; the
  sidebar, status bar and ⌘K / Ctrl+K palette carry the Pro entries and the licence read from
  the app. Without the shell (an older open tag) the window opens `pro/home.html`, the same
  home and licence screen on their own. The licence comes only from the app's commands
  (`licence_status`, `licence_activate`, `licence_deactivate`, `licence_machine_id`,
  `upgrade_offer`, `engine_call list_kinds`); a licence file is read in the page and activated
  with `licence_activate`, offline. States drawn: none, active, expiring, grace, expired,
  revoked, invalid, and "not read" outside the app. Tests: `node --test apps/desktop-pro/pro/desktop/*.test.mjs`.
- **Upgrade.** Shown only while payments are on in the build's
  `commercial/payments.toml`; with the committed configuration (off) there is no offer and
  no network call (a test checks it).

Build (needs a checkout of the open repository that carries the Desktop,
`web/studio/lib/desktop.mjs`):

```bash
apps/desktop-pro/build-frontend.sh <open repository checkout>
cargo test  --manifest-path apps/desktop-pro/src-tauri/Cargo.toml
cargo build --manifest-path apps/desktop-pro/src-tauri/Cargo.toml
(cd apps/desktop-pro/src-tauri && cargo tauri build)    # unsigned bundles
```

Not done: signing and notarisation (the publisher's Apple and Windows accounts); the
Devices panel (L2b) comes with the free Desktop.
