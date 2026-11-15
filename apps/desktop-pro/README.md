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
  the app. A shell with the open extension API (`window.KshanaShell`, the open
  `apps/desktop/README.md` "Extending the shell") gets these parts through it
  (`pro/desktop/hook.mjs`: a registered home that replaces the shell's home and upgrade
  views, a licence view, a sidebar section, a palette group, status items and the edition);
  an older shell without the API gets them by its markup, re-applied after each render
  (`overlay.mjs`). Without the shell (an older open tag) the window opens `pro/home.html`, the same
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

- **Pro work views** (0.42 U6, U19, U27, U28; `pro/desktop/work.mjs`, a "PRO WORK" sidebar
  section and palette group through the shell's extension API): Station fleet (concept B, from
  the Pro service's `/fleet/stations`), incident replay on the map (an E1 pack opened through
  `pro_incident_replay`: verified first, estimated vs reported tracks on one time cursor, trust
  shading from the pack's findings; J/K/L keys), review mode and report builder (signed with this
  device's evidence key through `pro_sign_files`; anyone verifies with `kshana-pro dossier
  verify` or `scripts/verify-dossier-signature.py`, no licence), receiver scorecards and
  multi-source trust (from the session's runs), area trust and the timing fleet (from the
  service), sign-in with an SSO token or API key (`pro_service_connect`; the app keeps the
  credential in an owner-only file and the page never sees it again), and, for Enterprise, the
  team library, the audit log and the watch wall. The app's `pro_` functions live in the Pro
  crate (`src/desktop/`, tested in `tests/desktop_pro.rs`); the bridge forwards them. Models
  and a render smoke test: `node --test apps/desktop-pro/pro/desktop/*.test.mjs`.

Not done: signing and notarisation (the publisher's Apple and Windows accounts); the
Devices panel (L2b) comes with the free Desktop.
